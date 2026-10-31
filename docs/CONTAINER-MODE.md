# EasyMC 容器模式（Podman）

切片 A–E（2026-10-30；C/D 合入 v0.1.8，E 合入 v0.1.9）。**原生路径永远可用**：`Instance.container = None` 表示非容器版本，行为与改前一致。

## 决策摘要

| 项 | 选择 |
| --- | --- |
| 运行时 | **Podman rootless**（Linux）；Windows GUI = **Podman Machine + WSL2 + WSLg** |
| macOS | **不走**容器 GUI 路径；Agent / 标准模式直接开玩 |
| Agent | 可与容器并存；**长期支持「仅 Agent、无容器」** |
| 镜像 | 公共标签（Temurin JRE 等）+ **中国镜像**（DaoCloud 公开前缀） |
| 安装 | 一键配置：**不** `curl \| sh`；返回可复制的发行版命令 / 官方 URL |
| 存档 | 默认提出建议（propose），不默认改动宿主存档 |
| Java / 环境 | 对中国网络友好（BMCLAPI + 容器 registries 镜像） |

MIT 友好复用：只采用 `registries.conf` **形态**与 DaoCloud **公开服务前缀**，不 vendor Podman / DaoCloud 源码。

## 切片 A — `easymc-container` + 引导

Crate：`crates/easymc-container/`

| API | 作用 |
| --- | --- |
| `probe_container_runtime` | 探测 Podman：`not_installed` / `installed_not_ready` / `ready` / `unsupported` |
| `install_guidance` / `apply_one_click_setup` | 一键配置计划 + 安全本地步骤（写 registries、尝试 `machine start`） |
| `write_china_registries_config` | 写入 `~/.config/containers/registries.conf.d/easymc-china.conf` |

设置字段（`Settings`）：

- `container_mode_enabled` — 玩家在引导里选择启用
- `container_china_mirrors` — 默认 `true`
- `container_setup_done` — 一键配置完成或跳过

引导：在 **模式选择（助手 / 标准）之后**插入「要用隔离环境吗？」一步，文案面向玩家；可「暂不使用」。

Tauri 命令：`container_probe_cmd`、`container_setup_plan_cmd`、`container_one_click_setup_cmd`、`container_write_registries_cmd`、…

## 切片 B — 版本元数据

`Instance.container: Option<ContainerMeta>`（定义在 `easymc-core`，避免环依赖）：

```text
runtime          // "podman"
container_id?    // podman 名称或 id
image?           // 基础镜像引用
volumes[]        // host → container 挂载（.minecraft rw；libraries/assets ro）
provisioned      // 是否已成功 podman create
```

| API | 作用 |
| --- | --- |
| `list_container_versions` | 列出带 container 元数据的实例 |
| `create_container_version` | 新建实例并 attach |
| `attach_container_to_instance` | 给已有实例挂上元数据 + best-effort `podman create` |
| `delete_container_version` | `podman rm -f` + 删实例行 |

首页徽章：有 `container` 时显示 **「容器」** chip（精选区与版本卡片）。

复制实例会 **清空** `container`（容器身份按版本独立，需重新 attach）。

## 默认基础镜像

- 公共：`docker.io/library/eclipse-temurin:21-jre`
- 国内：`docker.m.daocloud.io/library/eclipse-temurin:21-jre`

C 已接上运行时镜像拉取 + 容器内启动；完整 Mesa/GPU 镜像与 Win GUI 实机验收仍可迭代。

## 与 AgentLoop 的边界

- AgentLoop 已合入 main（v0.1.7）；C/D 对 `agent_loop.rs` 仅扩展危险工具名单（apply/discard layer、saves），不改循环结构。
- 推送前 `git pull --rebase`，勿覆盖 AgentLoop 提交。

## 玩家文案要点

- UI 说「版本隔离 / 隔离环境」，不说 Podman / WSL / registries（安装指引里最多点一次「官方安装包」）。
- 加速线路用「国内线路 A/B/C」「官方线路（海外）」，不说 DaoCloud / DockerProxy / BMCLAPI。
- Windows 开隔离环境需要官方安装包内的运行环境（含图形支持）；玩家文案勿堆术语。
- 「预热」对玩家说「提前准备 / 现在就准备好」。
- 环境包对玩家说「分享环境说明 / 分享完整环境 / 导入好友环境」。


## 切片 C — 启动路径（v0.1.8）

| API / 行为 | 作用 |
| --- | --- |
| `ensure_runtime_image` / `try_pull_runtime_image` | 拉取 Temurin JRE 镜像（DaoCloud / 官方回退） |
| `preheat_container_runtime` | 冷启动：Windows `machine start` + 可选拉镜像 |
| `default_volumes` + `remap_launch_args` | 宿主路径 → `/data/*`；classpath 在容器内统一 `:` |
| `plan_container_launch` / `plan_to_spawn` | 生成 `podman run --rm … image java …` |
| `prepare_and_launch` 分支 | `Instance.container` 有值时走容器启动（不依赖宿主 Java） |

挂载约定：

- `/data/.minecraft` ← 实例游戏目录（rw）
- `/data/libraries` / `assets` / `versions` / `natives` ← EasyMC 共享数据（ro）

游戏 jar / 库 / 资源仍由现有 `install_version`（BMCLAPI 感知）装到宿主卷；容器只消费这些卷。

**Linux**：优先验证 X11/Wayland + `/dev/dri`。  
**Windows 图形**：工程上依赖官方安装包的运行环境（含图形支持）；未确认图形支持的机器上可能仍阻塞，属已知限制。

Tauri：`container_provision_cmd`、`container_preheat_cmd`、`container_pull_image_cmd`。

## 切片 D — Agent 绑定（v0.1.8）

| API / 行为 | 作用 |
| --- | --- |
| `resolve_agent_workspace` | 容器版本 → Agent 根目录 = 游戏可写层（`.minecraft`） |
| `apply_game_layer` | **应用** = 将 mods/config/resourcepacks/shaderpacks/datapacks 快照为基线 |
| `discard_game_layer` | **丢弃** = 从基线重建上述目录（**永不**动 `saves/`） |
| `Settings.container_saves_require_propose` | 默认 `true`：存档修改需提出建议 |

`LAYER_DIRS` 不含 `saves/`。AgentLoop（`agent_loop.rs`）仅扩展危险工具名单（`apply_game_layer` / `discard_game_layer` / `modify_saves`），不改循环结构。

Tauri：`container_layer_*`、`container_agent_workspace_cmd`。  
UI：设置页「版本隔离」；版本管理设置里「隔离改动」应用/丢弃。

### C/D 加固（Focus B）

- `assemble_container_run_argv`：纯函数拼 `podman run` argv（含 remap），单测不依赖本机 Podman。
- Remap：`-cp`/`-classpath` 下一参数、`-D…=path` 多路径列表、Windows 盘符 classpath。
- GUI：X11 / Wayland 中文提示区分；挂载 `XAUTHORITY`；Windows 仅在无 `DISPLAY` 时补 `:0`。
- Layer：apply/discard 经 staging 再 rename，中途失败不破坏旧基线；`saves/` 仍不动。
- Agent：`path_is_gated_save` / `gate_saves_mutation`；危险工具卡中文说明；模组改动会 `mark_layer_dirty`。


## 玩家文案（C/D）

UI：设置「启动」→「版本隔离（可选）」；版本管理设置里「隔离改动」应用/丢弃。


- 「现在就准备好」— 提前准备隔离环境，开玩少等一会儿。
- 「应用更改 / 丢弃更改」— 像提交 / 回滚模组与配置，存档单独保护。
- 「存档修改需确认」— 默认开；可在设置关闭。

## 切片 E — 环境包 / 预热 / 镜像 / 修复（v0.1.9）

| API / 行为 | 作用 |
| --- | --- |
| `.easymc-env` 元数据 + `.easymc-env.zip` 完整包 | 导出容器版本的镜像偏好与模组/配置层（**不含存档**） |
| `export_env_pack` / `import_env_pack` | 导入创建新的容器版本（`Instance.container` 有值） |
| `preheat_status` / `preheat_container_runtime_ex` | 冷启动状态戳；可选强制拉镜像；Linux 轻量 socket 探测 |
| `Settings.container_auto_preheat` | 默认开：合适时机自动预热 |
| `MirrorProfile`（DaoCloud / 官方） | 设置页一键切换镜像源，写入 registries |
| `repair_container_runtime` | 一键修复：探测 → 写 registries → 预热/拉镜像 → 可选重新 provision |

Tauri：`container_export_env_cmd`、`container_import_env_cmd`、`container_preheat_status_cmd`、`container_preheat_ex_cmd`、`container_mirror_profiles_cmd`、`container_repair_cmd`。

UI：设置「版本隔离」→ 加速线路 / 后台准备 / 现在就准备好 / 应用加速配置 / 修复隔离环境；版本「发给好友」与「分享隔离环境」。

## 玩家文案（E）

UI：设置「版本隔离」→ 加速线路 / 后台准备 / 现在就准备好 / 应用加速配置 / 修复隔离环境；版本「发给好友」与「分享隔离环境」。


- 「导出环境 / 导入好友环境」— 把模组与配置层分享给好友，存档不会被带走。
- 「隔离环境下载加速」— 国内默认线路 A；网络正常可切官方线路。
- 「后台先准备好」— 提前把隔离环境备好，减少第一次开玩等待。
- 「修复隔离环境」— 出问题先点这个：探测、加速、准备一次跑完。

