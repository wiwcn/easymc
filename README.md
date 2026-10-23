# EasyMC

Open-source **Minecraft Java Edition** launcher — Tauri 2 + Lit (Web Components) + Vite.

**License:** MIT · **Milestone:** M0 scaffold (no real auth / download / launch yet)

## Stack

| Layer | Choice |
|-------|--------|
| Shell | [Tauri 2](https://v2.tauri.app/) |
| UI | [Lit 3](https://lit.dev/) Web Components |
| Bundler | Vite 6 |
| Backend | Rust Cargo workspace |
| Look | MD3-inspired **soft teal** seed |

### MD3 / Material choice (M0)

We use **thin custom CSS tokens** (`src/styles/tokens.css`) mapped to MD3 system color / shape / elevation names, plus hand-rolled Lit components.

**Why not `@material/web` alone?** That package is in maintenance mode. Depending solely on it for M0 would freeze styling. Token names stay MD3-compatible so an actively maintained Lit MD3 library can be swapped in later without renaming the design language.

## Quick start

Requires: **Node 20+**, **pnpm**, **Rust ≥ 1.90** (Tauri 2.12+; see `rust-toolchain.toml`), and Tauri [system dependencies](https://v2.tauri.app/start/prerequisites/) for a full native window.

**Linux packages (Debian/Ubuntu example):**

```bash
sudo apt install pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf libssl-dev build-essential
```

If the distro `rustc` is older than 1.90, install via [rustup](https://rustup.rs/).

```bash
cd /workspace/easymc   # or your clone path
pnpm install
pnpm build             # frontend only — must succeed
cargo check --workspace
```

### Frontend preview (no Tauri window)

```bash
pnpm dev               # http://localhost:1420
```

Launch button shows a stub toast. In browser preview the Rust `invoke` falls back to local sample data.

### Full desktop app

```bash
pnpm tauri dev
```

**System deps (Linux example):** `webkit2gtk-4.1`, `librsvg2`, `patchelf`, build essentials, `libssl-dev`, etc. See Tauri 2 prerequisites for your OS.

> On this scaffold host, WebKitGTK may be missing — `pnpm build` and `cargo check --workspace` are the M0 gates. `pnpm tauri build` / `dev` need those native libs.

## Crate map

```
easymc/
├── src/                      # Lit UI
├── src-tauri/                # Tauri entry (crate: easymc-app)
└── crates/
    ├── easymc-core           # Domain types, errors, sample instances
    ├── easymc-meta           # Java Edition version metadata provider trait
    ├── easymc-mods           # ModProvider trait + Modrinth / CurseForge stubs
    ├── easymc-agent          # LlmProvider trait (pluggable cloud) stub
    ├── easymc-share          # Instance share export/import stubs
    └── easymc-launch-desktop # Desktop launch stub (toast message only)
```

`src-tauri` depends on the workspace stubs and exposes `list_instances` / `launch_instance` / `app_info` commands.

## UI (M0)

Locked visual reference: `/workspace/easymc-mocks/` (`home-v1.jpg`, `store-v1.jpg`, `agent-v1.jpg`, `android-v1.jpg`).

- Left nav (Chinese): **启动 / 库 / 商店 / Agent / 分享 / 设置**
- Home: featured instance card + **启动** (stub toast) + instance cards
- Other routes: empty-state placeholders

Java Edition only. No Bedrock.

## Roadmap: M0 → M1

| | M0 (this) | M1 (next) |
|---|-----------|-----------|
| Auth | — | Microsoft / offline account |
| Versions | meta stub | Install JE from meta + libraries |
| Mods | Modrinth + CF trait stubs | Search + download + install |
| Launch | Stub toast | Real JVM spawn + logs |
| Agent | Cloud LLM trait stub | Wire first HTTP provider |
| Share | Bundle JSON stub | File / link share |
| Android | Out of M1 scope | Parallel track after desktop M1 |

## Scripts

| Command | Purpose |
|---------|---------|
| `pnpm dev` | Vite HMR |
| `pnpm build` | Production frontend → `dist/` |
| `pnpm tauri dev` | Native window + Vite |
| `pnpm tauri build` | Release bundles |
| `cargo check --workspace` | Type-check all crates |

## License

MIT — see [LICENSE](./LICENSE).
