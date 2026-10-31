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
console.log("ok — agent step abstracts");
