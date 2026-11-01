/** Real checks for batch mod-update helpers + Rust unit tests. */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = path.join(root, "src/lib/mod-updates.ts");
const out = path.join(root, "scripts/.cache-mod-updates.mjs");
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

assert.equal(mod.statusLabelZh("has_update"), "有更新");
assert.equal(mod.statusLabelZh("cf_needs_key"), "需 CurseForge Key");
assert.equal(mod.failReasonLabelZh("install_failed"), "安装失败");
assert.equal(mod.failReasonLabelZh("no_update"), "无需更新");

const cands = [
  {
    filename: "a.jar",
    disabled: false,
    source: "modrinth",
    project_id: "aaa",
    current_version_number: "1.0",
    latest_version_id: "v2",
    latest_version_number: "2.0",
    alternates: [
      { id: "v2", version_number: "2.0", name: "2.0" },
      { id: "v1.5", version_number: "1.5", name: "1.5" },
    ],
    status: "has_update",
  },
  {
    filename: "b.jar",
    disabled: false,
    source: "modrinth",
    project_id: "bbb",
    status: "up_to_date",
    alternates: [],
  },
  {
    filename: "c.jar",
    disabled: false,
    source: "curseforge",
    project_id: "ccc",
    status: "cf_needs_key",
    alternates: [],
  },
];

assert.deepEqual(mod.defaultSelectedFilenames(cands), ["a.jar"]);
assert.equal(mod.isSelectableUpdate(cands[0]), true);
assert.equal(mod.isSelectableUpdate(cands[2]), false);
assert.equal(mod.versionArrowLabel(cands[0]), "1.0 → 2.0");
assert.equal(mod.versionArrowLabel(cands[0], "v1.5"), "1.0 → 1.5");

const items = mod.buildApplyItems(cands, ["a.jar", "c.jar"], { "a.jar": "v1.5" });
assert.equal(items.length, 1);
assert.equal(items[0].version_id, "v1.5");
assert.equal(items[0].project_id, "aaa");

assert.ok(mod.changelogPreview("hello world ".repeat(40)).endsWith("…"));
assert.equal(mod.changelogPreview("short"), "short");

const rust = spawnSync(
  "cargo",
  ["test", "-p", "easymc-mods", "--lib", "update::"],
  { cwd: root, encoding: "utf8" },
);
if (rust.status !== 0) {
  console.error(rust.stdout);
  console.error(rust.stderr);
  process.exit(1);
}
assert.match(rust.stdout, /test result: ok/);

console.log("test-mod-updates: ok");
