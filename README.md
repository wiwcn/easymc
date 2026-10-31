# EasyMC

<p align="center">
  <img src="docs/assets/logo.svg" alt="EasyMC logo" width="96" height="96" />
</p>

<p align="center">
  <strong>开源 Minecraft Java Edition 启动器</strong><br />
  Open-source Minecraft Java Edition launcher<br />
  Tauri 2 · Lit · Vite · Rust
</p>

<p align="center">
  <a href="https://github.com/wiwcn/easymc/releases"><img alt="Release" src="https://img.shields.io/github/v/release/wiwcn/easymc?label=release" /></a>
  <img alt="License" src="https://img.shields.io/badge/license-LGPL--3.0--or--later-blue" />
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-teal" />
</p>

---

**中文** | [English](#easymc-english)

EasyMC 面向普通玩家：装好、启动、装模组、分享配置、排错——不是服务器运维面板。  
**桌面优先**（Windows / Linux / macOS）；Android 姊妹项目另见 `easymc-android`（真启动仍在推进）。只做 **Java Edition**。

| | |
|---|---|
| **仓库** | https://github.com/wiwcn/easymc |
| **许可** | [LGPL-3.0-or-later](./LICENSE)（见 [COPYING](./COPYING) / [NOTICE](./NOTICE)） |
| **版本** | 0.1.0 |

## 功能概览

按当前桌面端实际能力整理（不虚构未完成项）：

### 启动与版本
- 多实例 / 版本管理（对齐 HMCL 思路：选版本 → 大按钮启动）
- 原版安装：客户端、依赖库、natives、游戏资源；支持「快速安装（无资源）」
- 加载器：**Fabric / Quilt / Forge / NeoForge**（官方安装器路径）
- 系统 Java 检测；缺失时可下载 Temurin 到 `runtimes/`；设置里可选首选 Java
- **快速进入（Quick Play）**：世界 / 服务器 / Realms（1.20+ 官方参数；旧版服务器回退 `--server`/`--port`）

### 下载
- 下载源：**官方** 或 **BMCLAPI**（失败回退官方）；镜像 HTTPS 测速
- 模组商店：**Modrinth + CurseForge**（搜索 / 版本 / 安装）；已装模组在「库」里启用、禁用、卸载
- 整合包：Modrinth `.mrpack`、CurseForge 清单；下载中心可装整合包项目

### 账户与皮肤
- 离线账户；微软正版 Device Code 流程（需自备 `EASYMC_MSA_CLIENT_ID` 才完整可用）
- **皮肤衣柜**：3D 预览（skinview3d）、本地上传、正版名 / LittleSkin 导入；启动可套用当前皮肤
- 第三方皮肤站表单占位（完整 Authlib/Nide8 登录仍在规划）

### 助手（Agent）
- 默认 **本地规则引擎**（读日志 / 崩溃报告，中文诊断与修复建议）
- 可选 OpenAI 兼容 LLM（`EASYMC_LLM_*`）；危险操作 propose/confirm
- 工具卡：确保 Java、改内存、导出诊断包等

### 分享与排错
- 导出 / 导入：`.mrpack`、EasyMC share JSON / zip
- **mclo.gs**：一键上传 `latest.log` 或最新崩溃报告并复制链接
- 环境包 `.easymc-env`（容器相关配置导入导出，不含存档）

### 容器模式（可选）
- 默认仍是 **原生启动**；可在引导 / 设置中启用 Podman 容器隔离
- 探测、中国友好 registry 镜像、预热、可写层 Apply/Discard；Windows 依赖 WSL/WSLg，实机 GUI 可能仍受限  
  详见 [`docs/CONTAINER-MODE.md`](./docs/CONTAINER-MODE.md)

### UI
- Material Design 3 软青绿主题；侧栏：启动 / 下载 / 助手 / 联机 / 衣柜 / 设置 / 更多
- 产品设计见 [`DESIGN.md`](./DESIGN.md)

## 安装（Windows）

1. 打开 [Releases](https://github.com/wiwcn/easymc/releases) 下载最新 **`EasyMC_*_x64-setup.exe`**
2. 安装并运行（未代码签名时，SmartScreen 可能提示「仍要运行」）
3. 也可使用同版本附带的便携 **`easymc-app.exe`**

Linux / macOS：当前以从源码构建为主；CI 以 Windows NSIS 发布包为准。

## 从源码构建

需要：**Node 20+**、**pnpm**、**Rust ≥ 1.90**，以及 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/wiwcn/easymc.git
cd easymc
pnpm install
pnpm build
cargo check --workspace

# 开发 UI
pnpm tauri dev

# 生产包（示例：NSIS）
pnpm tauri build --bundles nsis
```

无头 CLI：

```bash
export EASYMC_DATA_DIR=/tmp/easymc-data   # 可选
cargo run -p easymc-cli -- ensure-java 21
cargo run -p easymc-cli -- install 1.21.1
cargo run -p easymc-cli -- launch default
```

更多 CLI（加载器、模组、Agent、分享）见下方 English 节与 `cargo run -p easymc-cli -- --help`。

## 配置要点

| 项 | 说明 |
|----|------|
| 数据目录 | `EASYMC_DATA_DIR` 或平台数据目录下的 `EasyMC` |
| 下载源 | 设置 → 下载源，或 `easymc download-source set bmclapi` |
| CurseForge | 设置中粘贴 API Key，或 `EASYMC_CURSEFORGE_API_KEY` |
| 微软登录 | `EASYMC_MSA_CLIENT_ID`（Azure 公共客户端） |
| Agent LLM | `~/.config/easymc/llm.env` 或 `EASYMC_LLM_BASE_URL` / `API_KEY` / `MODEL` |

## 仓库结构（简）

| 路径 | 角色 |
|------|------|
| `crates/easymc-core` | 类型、路径、账户、衣柜、设置 |
| `crates/easymc-meta` | 版本元数据与加载器安装 |
| `crates/easymc-launch-desktop` | Java、参数、进程 |
| `crates/easymc-mods` | Modrinth / CurseForge |
| `crates/easymc-agent` | 本地诊断 + 可选 LLM |
| `crates/easymc-share` | 分享 / `.mrpack` |
| `crates/easymc-container` | 可选 Podman 容器模式 |
| `crates/easymc-cli` | 无头 CLI |
| `src-tauri` | Tauri 应用 |
| `src/` | Lit 前端 |

## 已知缺口（诚实列表）

- 完整微软登录依赖你自备 Azure Client ID；皮肤站完整协议登录尚未完成
- NeoForge 对 MC 1.20.1 过渡坐标未接线（该版本请用 Forge）
- 容器模式在 Windows 上依赖 WSL 图形栈，体验因环境而异
- Android 真启动不在本仓库里程碑内

## 贡献

欢迎 Issue / PR。改 UI 请遵循 `DESIGN.md` 的玩家文案与 MD3 约束；大功能先对一下 `docs/FEATURE-BACKLOG.md`。提交说明可用中英双语。

## 许可

Copyright (c) 2026 EasyMC Contributors.

本项目以 **GNU Lesser General Public License v3.0 or later**（`LGPL-3.0-or-later`）发布：见 [LICENSE](./LICENSE)。LGPL 引用的 GPL-3.0 全文见 [COPYING](./COPYING)。

第三方依赖与模式复用保持其原许可（例如 npm 包 `skinview3d` 为 MIT）——详见 [NOTICE](./NOTICE)。

---

<a id="easymc-english"></a>

## EasyMC (English)

Open-source **Minecraft Java Edition** launcher (Tauri 2 + Lit + Vite + Rust). Built for players who want to install, launch, mod, share packs, and fix crashes—not run a server farm.

- **Desktop-first** (Windows / Linux / macOS). Android is a sibling track (`easymc-android`); real mobile launch is not claimed here.
- **Java Edition only.**

### Highlights (shipped)

- Multi-instance install & launch; Fabric / Quilt / Forge / NeoForge; Temurin/Java helpers  
- Download mirrors (official / BMCLAPI); Modrinth + CurseForge store; `.mrpack` / share import-export  
- Offline accounts; Microsoft device-code path (needs your `EASYMC_MSA_CLIENT_ID`); skin wardrobe with 3D preview  
- Local Agent diagnostics (+ optional OpenAI-compatible LLM); mclo.gs log upload; Quick Play  
- Optional Podman **container mode** (native launch remains default)—see `docs/CONTAINER-MODE.md`

### Install

Download the Windows NSIS installer from [Releases](https://github.com/wiwcn/easymc/releases) (`EasyMC_*_x64-setup.exe`). Unsigned builds may trip SmartScreen once.

### Build from source

Node 20+, pnpm, Rust ≥ 1.90, Tauri prerequisites:

```bash
pnpm install && pnpm build
cargo check --workspace
pnpm tauri dev
```

### License

**LGPL-3.0-or-later** — [LICENSE](./LICENSE), [COPYING](./COPYING). Third-party notices: [NOTICE](./NOTICE).

### Links

- Repository: https://github.com/wiwcn/easymc  
- Design: [`DESIGN.md`](./DESIGN.md)  
- Container mode: [`docs/CONTAINER-MODE.md`](./docs/CONTAINER-MODE.md)
