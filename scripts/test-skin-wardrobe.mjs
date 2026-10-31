/** Real checks for wardrobe frontend contracts (no mocks). */
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const viewer = fs.readFileSync(path.join(root, "src/components/skin-viewer.ts"), "utf8");
assert.match(viewer, /from "skinview3d"/);
assert.match(viewer, /WalkingAnimation/);
assert.match(viewer, /loadCape\(null\)/);
assert.match(viewer, /easymc-skin-viewer/);

const page = fs.readFileSync(path.join(root, "src/pages/wardrobe-page.ts"), "utf8");
assert.match(page, /皮肤衣柜/);
assert.match(page, /LittleSkin/);
assert.match(page, /wardrobeImportOfficial/);
assert.match(page, /pickSkinPng/);
assert.match(page, /easymc-skin-viewer/);

const api = fs.readFileSync(path.join(root, "src/api.ts"), "utf8");
for (const cmd of [
  "wardrobe_list_cmd",
  "wardrobe_upload_cmd",
  "skin_fetch_official_cmd",
  "skin_fetch_littleskin_cmd",
  "wardrobe_import_littleskin_cmd",
  "wardrobe_active_cmd",
  "wardrobe_stage_active_cmd",
]) {
  assert.match(api, new RegExp(cmd));
}

const types = fs.readFileSync(path.join(root, "src/types.ts"), "utf8");
assert.match(types, /wardrobe/);
assert.match(types, /WardrobeEntryDto/);
assert.match(types, /ResolvedSkinDto/);

const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
assert.equal(pkg.dependencies.skinview3d, "3.4.1");

const notice = fs.readFileSync(path.join(root, "NOTICE"), "utf8");
assert.match(notice, /skinview3d/);
assert.match(notice, /LittleSkin/);

const skinRs = fs.readFileSync(path.join(root, "crates/easymc-core/src/skin.rs"), "utf8");
assert.match(skinRs, /littleskin\.cn\/csl/);
assert.match(skinRs, /sessionserver\.mojang\.com/);
assert.match(skinRs, /bmclapi2\.bangbang93\.com/);

const wardRs = fs.readFileSync(path.join(root, "crates/easymc-core/src/wardrobe.rs"), "utf8");
assert.match(wardRs, /wardrobe_upload/);
assert.match(wardRs, /skin_data_url/);
assert.match(wardRs, /stage_active_for_launch/);
assert.match(wardRs, /active_wardrobe_dto/);


const face = fs.readFileSync(path.join(root, "src/lib/skin-face.ts"), "utf8");
assert.match(face, /skinFaceDataUrl/);
assert.match(face, /40 \* scale/);

const avatar = fs.readFileSync(path.join(root, "src/components/skin-avatar.ts"), "utf8");
assert.match(avatar, /skinDataUrl/);
assert.match(avatar, /skinFaceDataUrl/);

const launch = fs.readFileSync(path.join(root, "crates/easymc-launch-desktop/src/launch.rs"), "utf8");
assert.match(launch, /stage_active_for_launch/);
assert.match(launch, /CustomSkinLoader|CSL LocalSkin/);


const sideNav = fs.readFileSync(path.join(root, "src/components/side-nav.ts"), "utf8");
assert.match(sideNav, /label: "衣柜"/);
assert.match(sideNav, /id: "wardrobe"/);
assert.ok(sideNav.indexOf('id: "wardrobe"') < sideNav.indexOf('id: "settings"'), "衣柜 before 设置");
assert.ok(!/label: "账户"/.test(sideNav.split("const NAV")[1].split("];")[0]), "账户 not duplicated in NAV list");

const shell = fs.readFileSync(path.join(root, "src/app-shell.ts"), "utf8");
assert.match(shell, /fromAccounts/);
assert.match(shell, /wardrobeFromAccounts/);

console.log("ok — skin wardrobe contracts");
