# EasyMC Agent multi-step loop

## Goal

Upgrade the optional LLM path from **single tool_call** to a **pi-style AgentLoop**:
the model may call several tools across up to `max_turns` (default **8**, range 6–8),
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

`AgentLoopConfig.max_turns` / `DEFAULT_MAX_TURNS = 8` mirrors the OpenAI Agents
“run until done or hit max_turns” budget. `AskResponse.mode` becomes `llm+agent`
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
