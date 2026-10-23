# EasyMC

Open-source **Minecraft Java Edition** launcher — Tauri 2 + Lit + Vite + Rust.

**License:** MIT · **Milestone:** M1.4 (Modrinth + CurseForge mods)

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
- Mod loaders: **Fabric** (and **Quilt**) profile install via meta.fabricmc.net / meta.quiltmc.org; `inheritsFrom` merge; launch uses KnotClient + loader jars. Forge/NeoForge stubbed. UI create flow: 原版 | Fabric | Quilt
- Mod store: **Modrinth + CurseForge** search / version list / install jar into instance `mods/`. CLI + 商店 UI. CurseForge needs `EASYMC_CURSEFORGE_API_KEY`; Modrinth works without a key
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
cargo run -p easymc-cli -- launch default
```

### Desktop UI

```bash
pnpm tauri dev
```

Home: **安装并启动** / **启动** / **新建实例** (version + **加载器** 原版/Fabric/Quilt) + install progress bar. 库: same. **商店**: Modrinth/CurseForge 搜索安装到活动实例. Settings: offline accounts. Logs under the hero card. Toggle **下载游戏资源** or use **快速安装（无资源）** for faster iterate.

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
| `easymc-meta` | Piston-meta client, download, natives extract, assets |
| `easymc-launch-desktop` | Java detect/download, arg builder, process manager |
| `easymc-cli` | Headless install/launch |
| `easymc-mods` | Modrinth + CurseForge search/install |
| `easymc-agent` / `share` | M0 stubs |
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
- Forge/NeoForge install; full modpack import; mod update/remove UI polish
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
