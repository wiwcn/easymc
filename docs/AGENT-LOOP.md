# EasyMC Agent multi-step loop

## Goal

Upgrade the optional LLM path from **single tool_call** to a **pi-style AgentLoop**:
the model may call several tools across up to `max_turns` (default **64**, override with `EASYMC_AGENT_MAX_TURNS`),
the host executes **all** `tool_calls` in a turn, refeeds `role=tool` messages, and
stops on no tools / final text / `max_turns` / error.

The **rule-intent** path (`route` → `execute_intent`) is unchanged and still wins for
concrete Chinese utterances (装模组、版本列表、删除版本…).

## Loop (adapted from earendil-works/pi, MIT)

```
messages = [system(+skills), …history, user]
for turn in 1..=max_turns:
  completion = LLM(messages, tools=EasyMC schemas)
  if no tool_calls: stop (final_text / no_tools)
  append assistant(+tool_calls)
  for each tool_call:          # ALL of them
    if dangerous: propose/confirm card only (no side effect)
    else: route utterance → execute_intent → role=tool result
  continue
if max_turns after tools and empty prose: optional final complete_raw(tools=off)
```

Source inspiration: `https://github.com/earendil-works/pi` → `packages/agent/src/agent-loop.ts`
(attributed in `NOTICE` and `agent_loop.rs` header). EasyMC does not vendor the pi runtime.

## max_turns (OpenAI Agents SDK concept, MIT)

`AgentLoopConfig.max_turns` / `DEFAULT_MAX_TURNS = 64` (env `EASYMC_AGENT_MAX_TURNS`) mirrors the OpenAI Agents
“run until done or hit max_turns” budget, raised so dependency chains are not demo-capped. `AskResponse.mode` becomes `llm+agent`
when any tool card was produced.

Stop reasons returned on `AgentLoopResult.stop_reason`:
`final_text` | `no_tools` | `max_turns` | `error`.

## Skills (dsh-style, 1–3)

`skill_augmented_system_prompt(user)` picks up to **3** keyword-matched files from
`skills/*.md` and appends clipped bodies to the system prompt. Unrelated asks get
zero skill dump. Crash/diagnose prefers `agent-see-act-verify.md` (see → act → verify).

## Dangerous tools

`is_dangerous_tool`: delete_instance/version, delete/remove/uninstall_mod, toggle/enable/disable_mod,
bump_memory / apply_* — **never auto-run** in the LLM loop; return a pending card and tell the
model the player must confirm in Chinese so the rule path can execute.

## Files

| Path | Role |
|------|------|
| `crates/easymc-agent/src/agent_loop.rs` | Loop + gates |
| `crates/easymc-agent/src/skills.rs` | Skill inject |
| `crates/easymc-agent/src/llm.rs` | `complete_raw` + role=tool wire format |
| `crates/easymc-agent/src/service.rs` | `ask()` wires loop for Help/Unknown |
| `skills/*.md` | Skill bodies |
| `NOTICE` | Third-party attributions (EasyMC is LGPL-3.0-or-later) |

## UI

`AskResponse.tools: ToolCall[]` — Lit Agent page renders every card from a multi-step run;
`tool` remains the last/primary card for older clients.

`AskResponse.steps: AgentStepEvent[]` — short Chinese step abstracts (look → do → verify).
While `agent_ask` runs, the host also emits Tauri event `agent-step` with the same payload so
the chat can append lines live (`1. 正在检查模组依赖…`). Tool cards stay collapsible detail;
the step list leads. **After the turn finishes**, the Agent page folds all step abstracts into a
`<details>` summary（「已执行 N 步 · 点击展开」）; live progressive steps stay open during the run.

Final `reply_zh` prefers a plain-Chinese **原因 / 方案** block derived from tool payloads
(`compose_reason_plan_zh`: missing deps, fill ok/fail, conflicts) instead of a bare「好了。」.

## Mod dependency tools

Fuzzy player asks like 「缺依赖」「能不能补上」「缺啥」route to `CheckModDeps` (rules) or, on the LLM path, should call:

1. `check_mod_deps` — local jar metadata → missing ids / conflicts / tree excerpt
2. `fetch_missing_mod_deps` — host downloads via Modrinth (`fetch_missing_deps`); confirm not required (install into mods/), same pattern as `install_mod`

Skill: `skills/agent-mod-deps.md`. Do not stop after `list_installed_mods` alone.

## Full pi kernel vs EasyMC AgentLoop

William authorized vendoring the full **earendil-works/pi** agent runtime (MIT) if the
thin loop could not do multi-step deps reliably. **Path taken for v0.1.2:** keep the
existing Rust `agent_loop` (already adapted from `packages/agent/src/agent-loop.ts`:
all tool_calls per turn, `role=tool` refeed, `max_turns` ≥ 64, skill inject) and
**wire real dependency tools** instead of dropping in the TypeScript pi package.

Root cause of the「缺依赖」failure was not a broken loop budget — the model only had
`list_installed_mods`, so it listed jars and stopped. Fix: `check_mod_deps` /
`fetch_missing_mod_deps` + `skills/agent-mod-deps.md` + rule intent for 缺依赖.
No `vendor/pi` tree added; NOTICE still attributes the adapted loop concept.

