/** Real unit checks for download/home UX helpers (no mocks). */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = path.join(root, "src/lib/download-ux.ts");
const out = path.join(root, "scripts/.cache-download-ux.mjs");
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

assert.equal(mod.marketInstallCta("mod"), "装进当前版本");
assert.equal(mod.marketInstallCta("modpack"), "安装整合包");
assert.equal(mod.marketInstallCta("shader"), "下载光影");
assert.equal(mod.marketNeedsInstance("mod"), true);
assert.equal(mod.marketNeedsInstance("modpack"), false);
assert.equal(mod.marketEmptyTitle(" neo ").title.includes("搜到"), true);
assert.equal(mod.marketEmptyTitle("").title.includes("热门"), true);
assert.deepEqual(mod.filterVersionIds(["1.21.1", "1.20.4", "24w14a"], "1.21"), ["1.21.1"]);
assert.deepEqual(mod.filterVersionIds(["1.21.1"], "  "), ["1.21.1"]);
assert.deepEqual(
  mod.sortInstancesReadyFirst([{ id: "a", version_ready: false }, { id: "b", version_ready: true }]).map((x) => x.id),
  ["b", "a"],
);
console.log("ok — download-ux helpers");
