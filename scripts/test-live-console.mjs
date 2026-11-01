/** Real unit checks for live console helpers (no mocks). */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = path.join(root, "src/lib/live-console.ts");
const out = path.join(root, "scripts/.cache-live-console.mjs");
fs.mkdirSync(path.dirname(out), { recursive: true });

function findEsbuild() {
  const direct = path.join(root, "node_modules/esbuild/bin/esbuild");
  if (fs.existsSync(direct)) return direct;
  const pnpm = path.join(root, "node_modules/.pnpm");
  if (!fs.existsSync(pnpm)) return null;
  for (const d of fs.readdirSync(pnpm)) {
    if (!d.startsWith("esbuild@")) continue;
    const cand = path.join(pnpm, d, "node_modules/esbuild/bin/esbuild");
    if (fs.existsSync(cand)) return cand;
  }
  return null;
}

const esbuildBin = findEsbuild();
if (!esbuildBin) {
  console.error("esbuild not found");
  process.exit(1);
}
const build = spawnSync(
  esbuildBin,
  [src, "--bundle", "--platform=node", "--format=esm", `--outfile=${out}`],
  { cwd: root, encoding: "utf8" },
);
if (build.status !== 0) {
  console.error(build.stderr || build.stdout || "esbuild failed");
  process.exit(1);
}

const mod = await import(out + `?t=${Date.now()}`);

assert.equal(mod.lineSeverity("[21:00:01] [main/ERROR]: boom"), "error");
assert.equal(mod.lineSeverity("java.lang.NullPointerException"), "error");
assert.equal(mod.lineSeverity("[main/WARN]: slow"), "warn");
assert.equal(mod.lineSeverity("[main/INFO]: hello"), "info");

const lines = [
  "[INFO]: boot",
  "[ERROR]: fail A",
  "[WARN]: soft",
  "[ERROR]: fail B",
  "Caused by: oops",
];
assert.deepEqual(
  mod.filterLogLines(lines, "fail", "all"),
  ["[ERROR]: fail A", "[ERROR]: fail B"],
);
assert.deepEqual(
  mod.filterLogLines(lines, "", "error"),
  ["[ERROR]: fail A", "[ERROR]: fail B", "Caused by: oops"],
);
assert.deepEqual(mod.filterLogLines(lines, "soft", "warn"), ["[WARN]: soft"]);
assert.deepEqual(mod.filterLogLines(lines, "zzz", "all"), []);

let buf = [];
for (let i = 0; i < 5; i++) buf = mod.appendLiveLog(buf, `L${i}`, 3);
assert.deepEqual(buf, ["L2", "L3", "L4"]);

assert.equal(mod.joinLogText(["a", "b"]), "a\nb");

const empty = mod.consoleEmptyHint({ hasFilter: false });
assert.ok(empty.title.includes("没有") || empty.title.includes("还没有"));
const filtered = mod.consoleEmptyHint({ hasFilter: true });
assert.ok(filtered.detail.includes("筛选") || filtered.detail.includes("关键词"));

assert.equal(
  mod.shouldStickToBottom({ scrollTop: 100, clientHeight: 50, scrollHeight: 160 }, 48),
  true,
);
assert.equal(
  mod.shouldStickToBottom({ scrollTop: 0, clientHeight: 50, scrollHeight: 400 }, 48),
  false,
);

console.log("ok — live-console helpers");
