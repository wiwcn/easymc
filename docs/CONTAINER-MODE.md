# EasyMC 容器模式（Podman）

切片 A + B（2026-10-30 批准决策）。**原生路径永远可用**：`Instance.container = None` 表示非容器版本，行为与改前一致。

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

引导：在 **模式选择（Agent / 标准）之后**插入「要用容器隔离吗？」一步，文案面向玩家；可「暂不使用」。

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

完整游戏 + Mesa / GPU 直通镜像属后续切片；A/B 只保证探测、配置、元数据与创建/删除。

## 与 AgentLoop 的边界

- AgentLoop 已合入 main（v0.1.7）；容器工作 **不修改** `crates/easymc-agent/src/agent_loop.rs`，除非必须动共享文件并小心合并。
- 推送前 `git pull --rebase`，勿覆盖 AgentLoop 提交。

## 玩家文案要点

- 容器 = **内容隔离**（mods/存档按版本分开），不是「更安全的沙箱」承诺。
- Windows：需要 Podman Machine（WSL2）+ WSLg 才能走 GUI 接受路径。
- 不装容器也能完整使用 Agent 与标准模式。
