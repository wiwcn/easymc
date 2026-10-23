# EasyMC

Open-source **Minecraft Java Edition** launcher — Tauri 2 + Lit + Vite + Rust.

**License:** MIT · **Milestone:** M1.7 (配置一键分享 — export/import)

**Android track:** sibling repo [`/workspace/easymc-android`](../easymc-android) (A2 — MIT EGL Surface + natives staging; see its DESIGN.md). Not the same milestone as desktop M1.

## What works (M1)

- Local JSON persistence for instances & accounts (`EASYMC_DATA_DIR` or platform data dir `/EasyMC`)
- Default offline account (`EasyPlayer`) + Settings UI to create more
- Mojang piston-meta: list / resolve / install client + libraries + natives (+ assets)
- System Java detection; Temurin JDK download into `runtimes/` when missing
- Desktop launch: classpath, natives, argument templates, process + log streaming
- Home **启动** wires to real install/launch; log panel in UI
- In-UI version list (release / snapshot toggle) + create named instance
- Multi-instance: list, set default, launch selected (Home + 库)
- Install progress: phase labels (元数据/客户端/依赖库/本地库/游戏资源) + counts/%; CLI stderr lines; Tauri `install-progress` events → Home/库 progress bar
- Skip-if-present clarity (新下 / 跳过); `--no-assets` + UI「下载游戏资源」/「快速安装（无资源）」; mid-fail drops `.part` and surfaces stage-scoped errors
- Mod loaders: **Fabric** / **Quilt** (meta profile JSON) + **Forge** / **NeoForge** (official installer JAR `--installClient`, processors run under Temurin). `inheritsFrom` merge; launch resolves profile mainClass/libraries. UI create chips: 原版 | Fabric | Quilt | Forge | NeoForge
- Mod store: **Modrinth + CurseForge** search / version list / install jar into instance `mods/`. CLI + 商店 UI. CurseForge needs `EASYMC_CURSEFORGE_API_KEY`; Modrinth works without a key
- **Share (M1.7):** export/import instance configs — metadata JSON or pack zip with `mods/*.jar`. Resolves missing mods via Modrinth/CF ids when known. CLI `export-instance` / `import-instance`; UI on 库 / 启动
- Microsoft auth: structure + clear TODO (`EASYMC_MSA_CLIENT_ID`); offline play works without it
- Headless CLI: `cargo run -p easymc-cli -- <cmd>`

## Quick start

Requires Node 20+, pnpm, **Rust ≥ 1.90**, and [Tauri Linux deps](https://v2.tauri.app/start/prerequisites/).

```bash
cd /workspace/easymc
pnpm install && pnpm build
cargo check --workspace
```

### CLI smoke (recommended first)

```bash
export EASYMC_DATA_DIR=/tmp/easymc-data   # optional
cargo run -p easymc-cli -- ensure-java 21
cargo run -p easymc-cli -- install 1.21.1          # full assets ~800MB+; progress on stderr
# faster iterate without assets (title screen may fail):
# cargo run -p easymc-cli -- install 1.21.1 --no-assets
cargo run -p easymc-cli -- list-loaders fabric 1.21.1
cargo run -p easymc-cli -- install-fabric 1.21.1 --no-assets
cargo run -p easymc-cli -- create-instance "Fabric" 1.21.1 --loader fabric --loader-version 0.20.1
cargo run -p easymc-cli -- list-loaders neoforge 1.21.1
cargo run -p easymc-cli -- install-neoforge 1.21.1 --no-assets
# cargo run -p easymc-cli -- install-forge 1.21.1 --loader 52.1.20 --no-assets
cargo run -p easymc-cli -- create-instance "NeoForge" 1.21.1 --loader neoforge --loader-version 21.1.264
cargo run -p easymc-cli -- search-mods modrinth sodium --game 1.21.1 --loader fabric
cargo run -p easymc-cli -- install-mod modrinth AANobbMI --instance <fabric-instance-id>
# CurseForge (needs key):
# export EASYMC_CURSEFORGE_API_KEY=...
# cargo run -p easymc-cli -- search-mods curseforge sodium --game 1.21.1 --loader fabric
cargo run -p easymc-cli -- curseforge-key
cargo run -p easymc-cli -- list-versions          # releases
# cargo run -p easymc-cli -- list-versions --snapshots
cargo run -p easymc-cli -- create-instance "速通" 1.20.1
cargo run -p easymc-cli -- instances
cargo run -p easymc-cli -- set-active default
cargo run -p easymc-cli -- export-instance <instance-id>              # metadata .json
cargo run -p easymc-cli -- export-instance <instance-id> --pack       # zip + mods
cargo run -p easymc-cli -- import-instance /path/to/share.json
# cargo run -p easymc-cli -- import-instance /path/to/pack.zip --name "共享包" --apply-memory
cargo run -p easymc-cli -- launch default
```

### Desktop UI

```bash
pnpm tauri dev
```

Home: **安装并启动** / **启动** / **新建实例** (version + **加载器** 原版/Fabric/Quilt/Forge/NeoForge) + install progress bar + **导出元数据/整合包** / **导入分享**. 库: same + per-card export. **商店**: Modrinth/CurseForge. **Agent**: diagnose + optional LLM. Settings: offline accounts. Toggle **下载游戏资源** or **快速安装（无资源）**.


## Agent (M1.6)

Local-first MC assistant. **Default = offline rule engine** (no network to LLM providers).

### What it does

1. **Diagnose** — reads process logs and/or `instances/<id>/.minecraft/logs/latest.log` (+ newest crash-report), matches common patterns (OOM, missing/wrong Java, Mixin conflict, wrong loader, OpenGL/GLFW, OpenAL, auth, duplicate module…) and prints **zh-CN** summaries + fixes.
2. **Safe actions** — ensure Java (via existing `ensure-java`), bump `settings.memory_mb`, show data dir, export a diagnostic txt under `diagnostics/`.
3. **Intent router** — keyword/phrase mapping for e.g. 「看上次报错」「确保 Java 21」「内存 4G」「装 sodium」「数据目录」「导出诊断包」. Not an LLM.
4. **Optional LLM** — if `EASYMC_LLM_BASE_URL` + `EASYMC_LLM_API_KEY` are set, free-form chat and smarter log analysis go through an **OpenAI-compatible** `/v1/chat/completions` client (e.g. DeepSeek via a gateway). Secrets stay in env / `~/.config/easymc/llm.env` — never committed.

Stretch: `LocalGgufBackend` trait scaffold exists but is **disabled** by default.

### CLI

```bash
cargo run -p easymc-cli -- agent diagnose --instance default
cargo run -p easymc-cli -- agent diagnose --bundle
cargo run -p easymc-cli -- agent ask "看上次报错"
cargo run -p easymc-cli -- agent ask "确保 Java 21"
cargo run -p easymc-cli -- agent ask "你好，帮我看看怎么装 iris"   # needs LLM env
cargo run -p easymc-cli -- agent ask "随便聊聊" --rules-only       # force offline
cargo run -p easymc-cli -- agent status
```

### Optional LLM env

```bash
# recommended file (auto-loaded if present; see .env.example):
# ~/.config/easymc/llm.env
export EASYMC_LLM_BASE_URL=https://api.example.com/v1
export EASYMC_LLM_API_KEY=sk-...
export EASYMC_LLM_MODEL=deepseek-chat   # or deepseek-reasoner, etc.
```

UI **Agent** page: diagnose button, action chips, chat composer (LLM when configured).

## Share (M1.7)

Portable instance configs — **no full assets/client jars** by default.

### Modes

| Mode | File | Contents |
|------|------|----------|
| **Metadata** | `*.easymc.share.json` (or `easymc.share.json`) | Version, loader+version, JVM memory suggestion, mods list (filename + optional Modrinth/CF ids / sha1) |
| **Pack** | `*.easymc.share.zip` | Same manifest at `easymc.share.json` + `mods/*.jar` |

Format id: `easymc.share` · `format_version: 1`. Optional local index `.easymc-mods.json` under the instance caches Modrinth/CF ids after store installs.

### Import behaviour

1. Create (or update by name) an instance from the manifest.
2. For each mod: if jar present in pack → copy; else if Modrinth/CF ids present → download; else record as missing.
3. `--apply-memory` / UI confirm optionally writes `settings.memory_mb`.
4. Does **not** download Minecraft client/assets; install the version separately.

### CLI

```bash
cargo run -p easymc-cli -- export-instance <id> [--pack|--metadata] [--out PATH] [--notes TEXT] [--no-resolve]
cargo run -p easymc-cli -- import-instance <file.json|file.zip> [--name NAME] [--apply-memory] [--offline]
```

Default export dir: `$EASYMC_DATA_DIR/shares/` (or platform data dir). Path is printed and can be copied from the UI clipboard.


### Forge / NeoForge caveats

- Install uses the **official installer** (`java -jar *-installer.jar --installClient <EASYMC_DATA_DIR>`), so binarypatcher and other processors run as upstream intends. Requires a working Temurin/Java (auto `ensure-java 21`).
- Writes `launcher_profiles.json` under the data dir if missing (installer requirement).
- **NeoForge** versioning: MC `1.21.1` → builds `21.1.*` (`net.neoforged:neoforge`). Profile id = `neoforge-{build}` (e.g. `neoforge-21.1.264`). Supported for MC **≥ 1.20.2**. MC **1.20.1** used a transitional GAV (`net.neoforged:forge`) and is **not** wired yet — use Forge for 1.20.1.
- **Forge** maven version = `{mc}-{build}` (e.g. `1.21.1-52.1.20`); EasyMC stores `loader_version` as the build only (`52.1.20`). Profile id = `{mc}-forge-{build}`.
- First install downloads the installer into `meta/installers/` and can take several minutes (patching). Re-runs skip present libraries.
- Launch needs the same display/GPU caveats as vanilla; offline auth works.
- Classpath: EasyMC dedupes inherited libraries by Maven name and **omits the vanilla client jar** for BootstrapLauncher/ForgeBootstrap (FML loads the patched client). Fabric/Quilt still get the vanilla jar.

### CurseForge API Key

Official CurseForge API requires a key from [console.curseforge.com](https://console.curseforge.com/). Set:

```bash
export EASYMC_CURSEFORGE_API_KEY=your_key_here
```

Without it, Modrinth still works; CurseForge search/install returns a clear zh-CN error. User-Agent identifies as EasyMC.

## Crate map

| Crate | Role |
|-------|------|
| `easymc-core` | Types, paths, JSON store, offline UUID, MSA config stub |
| `easymc-meta` | Piston-meta + Fabric/Quilt meta + Forge/NeoForge official installer |
| `easymc-launch-desktop` | Java detect/download, arg builder, process manager |
| `easymc-cli` | Headless install/launch |
| `easymc-mods` | Modrinth + CurseForge search/install |
| `easymc-agent` | Local diagnose + optional OpenAI-compatible LLM |
| `easymc-share` | Export/import share bundles (metadata JSON / pack zip) |
| `src-tauri` (`easymc-app`) | Tauri commands + events |

## MD3 UI

Thin custom CSS tokens (`src/styles/tokens.css`), soft teal seed. Avoids sole dependence on maintenance-mode `@material/web`.

Nav: **启动 / 库 / 商店 / Agent / 分享 / 设置**

## Microsoft auth TODO

1. Register Azure public client app  
2. Set `EASYMC_MSA_CLIENT_ID`  
3. Implement device-code or localhost redirect → Xbox Live → XSTS → Minecraft Services profile/token  
4. Store refresh token in `accounts.json`

Until then use **离线账户**.

## Smoke status (this box)

Verified with `EASYMC_DATA_DIR=/workspace/easymc-data`:

- `ensure-java 21` → Temurin 21.0.13
- `install 1.21.1` → client + 56 libs + natives + asset index 17 (3911 objects)
- `launch default` → LWJGL 3.3.3, offline user EasyPlayer, texture atlases created (title-screen path). OpenAL/ALSA missing on headless audio → game continues muted. Needs a real DISPLAY/GPU for the window.

## Remaining gaps for “actually usable”

- Full MSA login (needs Azure client id + OAuth → Xbox → Minecraft services finish)
- Mod update/remove UI polish; NeoForge 1.20.1 transitional artifact; cloud pastebin hosting (local path share is enough)
- Asset download progress UI polish; optional mirror CDNs
- Windows JDK zip extract path (Linux tar.gz path works)
- Real audio device for OpenAL (optional)
- GPU/display for the game window itself

## Linux packages

```bash
sudo apt install pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf libssl-dev build-essential
```

## License

MIT — see [LICENSE](./LICENSE).
