/** Contract checks for Agent progressive step abstracts. */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const page = readFileSync(path.join(root, "src/pages/agent-page.ts"), "utf8");
const api = readFileSync(path.join(root, "src/api.ts"), "utf8");
const types = readFileSync(path.join(root, "src/types.ts"), "utf8");
const loop = readFileSync(path.join(root, "crates/easymc-agent/src/agent_loop.rs"), "utf8");

assert.match(types, /export interface AgentStepDto/);
assert.match(api, /listenAgentStep/);
assert.match(api, /"agent-step"/);
assert.match(page, /renderStepAbstracts/);
assert.match(page, /step-abstracts/);
assert.match(page, /listenAgentStep/);
assert.match(loop, /struct AgentStepEvent/);
assert.match(loop, /run_agent_loop_with_progress/);
assert.match(loop, /tool_start_summary_zh/);
assert.ok(page.includes("执行步骤"), "aria-label for steps");

assert.match(page, /step-abstracts-fold/);
assert.match(page, /已执行/);
assert.match(page, /composeReasonPlanFromTools/);
assert.match(page, /原因/);
assert.match(page, /方案/);
assert.match(loop, /compose_reason_plan_zh/);
assert.match(loop, /derive_reason_plan/);
assert.ok(page.includes('t.mode !== "running"') || page.includes("t.mode !== 'running'"), "fold when not running");

console.log("ok — agent step abstracts + fold + 原因/方案");
