# EasyMC

Open-source **Minecraft Java Edition** launcher — Tauri 2 + Lit + Vite + Rust.

**License:** MIT · **Milestone:** M1 slice (install + offline launch path)

## What works (M1)

- Local JSON persistence for instances & accounts (`EASYMC_DATA_DIR` or platform data dir `/EasyMC`)
- Default offline account (`EasyPlayer`) + Settings UI to create more
- Mojang piston-meta: list / resolve / install client + libraries + natives (+ assets)
- System Java detection; Temurin JDK download into `runtimes/` when missing
- Desktop launch: classpath, natives, argument templates, process + log streaming
- Home **启动** wires to real install/launch; log panel in UI
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
cargo run -p easymc-cli -- install 1.21.1          # full assets ~800MB+
# faster iterate without assets (title screen may fail):
# cargo run -p easymc-cli -- install 1.21.1 --no-assets
cargo run -p easymc-cli -- launch default
```

### Desktop UI

```bash
pnpm tauri dev
```

Home: **安装并启动** / **启动**. Settings: offline accounts. Logs stream under the hero card.

## Crate map

| Crate | Role |
|-------|------|
| `easymc-core` | Types, paths, JSON store, offline UUID, MSA config stub |
| `easymc-meta` | Piston-meta client, download, natives extract, assets |
| `easymc-launch-desktop` | Java detect/download, arg builder, process manager |
| `easymc-cli` | Headless install/launch |
| `easymc-mods` / `agent` / `share` | M0 stubs (next milestones) |
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

## Remaining gaps for “actually usable”

- Full MSA login (needs your Azure client id + OAuth implementation finish)
- In-UI version picker / multi-instance create (CLI works today)
- Mod loaders (Fabric/Forge) — vanilla only for this slice
- Asset download progress UI polish; optional mirror CDNs
- Windows JDK zip extract path
- Signed launcher branding / demo mode flags
- GPU/display required for the game window itself

## Linux packages

```bash
sudo apt install pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf libssl-dev build-essential
```

## License

MIT — see [LICENSE](./LICENSE).
