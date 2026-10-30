# EasyMC 每日工作规划

时区：Asia/Shanghai。推送只用本机 `~/.config/easymc/gh_token`。不主动打扰 William（除非被挡住）。


<!-- CONTAINER-MODE-A-B -->

## 已完成 · 镜像测速 + 引导页固定布局（v0.1.11 · 2026-10-30）

- 下载源 / 容器 registry 多候选 + 延迟探测，引导页与设置可选最快
- 引导页：顶栏进度+标题固定、底部「下一步」固定、中部可滚动
- `container_mirror_profile`：daocloud / dockerproxy / onems / official

## 已完成 · 环境包文件选择器（v0.1.10 · 2026-10-30）

- 版本管理「导入环境包」：原生 Tauri 文件选择器（`.easymc-env` / `.zip`），不再用手输路径。
- 约束：未改 AgentLoop / 容器核心；玩家中文文案。

## 已完成 · 容器 E（v0.1.9 · 2026-10-30）

- **E**：`.easymc-env` 环境导出/导入（不含存档）；预热状态/戳 + 强制拉镜像；镜像源档案（DaoCloud/官方）；一键修复；`container_auto_preheat`。
- UI：设置页镜像源/自动预热/修复；版本管理环境包导入导出。玩家中文文案。
- 约束：AgentLoop 未改结构；`Instance.container = None` 原生路径不变。

## 已完成 · 容器 C/D（v0.1.8 · 2026-10-30）

- **C**：Temurin JRE 镜像（DaoCloud）拉取、卷挂载、`podman run` 启动路径、预热；Linux 优先，Win = Machine+WSLg 设计（GUI 实机可能仍阻塞）。
- **D**：Agent 工作区 = 容器可写层；Apply/Discard 快照语义；`container_saves_require_propose` 默认开；AgentLoop 仅危险工具名单扩展。
- 文档：`docs/CONTAINER-MODE.md`；设置页 + 版本管理「容器可写层」。

## 容器模式切片 A–E（已合入 v0.1.9）

- **Crate** `easymc-container`：Podman 探测、一键配置（无 curl|sh）、中国 registries drop-in、版本 create/attach/list/delete。
- **Core**：`ContainerMeta` + `Settings.container_*`；`Instance.container`；duplicate 清空容器身份。
- **UI**：引导在 Agent/模式后询问容器 + 一键配置；首页「容器」徽章。
- **文档**：`docs/CONTAINER-MODE.md`。
- **C/D**：镜像拉取、容器启动、预热；Agent 工作区=可写层；Apply/Discard；存档默认 propose。
- **约束**：AgentLoop 仅危险工具名单扩展；推送前 rebase；原生路径不变。

## 参照启动器（功能/IA，不是视觉抄袭）

核心对齐：
- [HMCL](https://github.com/HMCL-dev/HMCL) — 启动页 / 版本与实例管理逻辑偏它
- [PCL](https://github.com/Meloong-Git/PCL) — 下载、账户、整体功能面全面对齐（有的都要有）
- [SJMCL](https://github.com/UNIkeEN/SJMCL) — Tauri 跨平台、实例中心、Modrinth+CF、扩展/Agent 协作（重点参考）
- [FCL / FoldCraftLauncher](https://github.com/FCL-Team/FoldCraftLauncher) — Android Java 真启动与运行时（桌面优先时后置，但能力要规划）

可借鉴（GitHub 常搜）：
- [PrismLauncher](https://github.com/PrismLauncher/PrismLauncher) — 多实例、导入、资源管理
- [PandoraLauncher](https://github.com/Moulberry/PandoraLauncher) — 现代实例管理
- [scl](https://github.com/Steve-xmh/scl) — Rust 启动器框架
- [quantumlauncher](https://github.com/Mrmayman/quantumlauncher)、[basalt-launcher](https://github.com/MegalithOfficial/basalt-launcher)、[nitrolaunch](https://github.com/Nitrolaunch/nitrolaunch)、[shard](https://github.com/Th0rgal/shard)
- 其它 Tauri/Rust JE 启动器：定期 `gh search` 再扫一轮，有用的能力记进缺口清单

UI：可对齐参照的**页面逻辑与布局规范**（得体，不搬具体文案/素材/品牌皮）；日后做视觉对齐。Impeccable 仍是质量底线（与个人偏好冲突时以 Impeccable 为准）。

## 固定节奏

| 时间 | 事项 |
|------|------|
| 工作日约 10:00 | 例行开发：拉最新 → 对照参照/缺口清单推进 → Agent 工具/skill → 有实质改动则打 Windows 包 |
| 每周至少一次 | GitHub 再搜 JE 启动器，更新「可借鉴」与缺口 |
| 按需 | 定稿后更新 Release（当前线：v0.1.11） |

## 每日清单（按优先级）

1. **对齐**：`git pull` main，看其他协作者昨夜改了什么（含 `trae/*` 等分支若对方尚未合入）。
2. **功能对齐**：对照 PCL / HMCL / SJMCL（及本周搜到的项目）推进缺口；启动页偏 HMCL，其余偏 PCL；SJMCL 重点学跨平台 IA 与 Agent/扩展协作。
3. **Agent**：更多 MC 真工具 + 完整工具卡；稳定流程沉淀为 skill。
4. **整合包**：保持 `.mrpack` 导入/导出可用。
5. **打包**：当日有用户可见/可发布提交 → Windows NSIS + 更新 GitHub Release。
6. **收尾**：本对话短报进度与明日重点；无事则安静。

## 本周主题（滚动）

- P0：PCL / HMCL / SJMCL 缺失功能清单 → 逐项落地
- P0：Agent 工具面与 skill 库（启用/禁用模组、版本列表、导入 .mrpack 等）
- P1：`.mrpack` / 分享链路打磨
- P1：Windows 定稿包常绿
- P2：皮肤站、Android 真启动（桌面优先时后置；FCL 作能力参照）

## 不做

- 不写 MCBBS
- 不用 Cursor（CloudAgent / GitHub MCP / 付费后台助手）；本机 Shell + gh_token 即可
- 不把密钥写进 git
- 不搬参照启动器的具体文案、素材与品牌皮；布局/IA 可对齐，视觉对齐后续推进

## 周扫描备忘（2026-10-28）

核心参照仍活跃：Prism / HMCL / PCL / FCL / SJMCL / Pandora；`scl` 已 archived。

本周值得留意（能力借鉴，不抄皮）：
- [OneLauncher](https://github.com/Polyfrost/OneLauncher) — 纯 Rust monorepo（launcher + client core）
- [PandoraLauncher](https://github.com/Moulberry/PandoraLauncher) — 现代实例管理（持续更新）
- [quantumlauncher](https://github.com/Mrmayman/quantumlauncher)、[nitrolaunch](https://github.com/Nitrolaunch/nitrolaunch)、[portablemc](https://github.com/theorzr/portablemc)
- Tauri 同族：plume-launcher、swift-client、cookie-launcher、nexus-launcher
- 另见 horizon-rust（服务端向，非启动器主参照）

EasyMC 缺口对照（滚动）：实例导入多样性（Prism/Pandora）、Agent/扩展协作（SJMCL）、游戏内文件夹快捷入口（已补 mods/资源包/光影/存档/配置/日志/崩溃报告）、版本重命名/复制/删除与模组删除（2026-10-30 Agent）、Android 真启动（FCL，桌面优先后置）。


## 日更备忘（2026-10-30）

- Agent：**pi-style AgentLoop**（max_turns 8，执行全部 tool_calls，role=tool 回灌；危险工具 propose/confirm）+ skill 1–3 注入；NOTICE 标注 pi / OpenAI Agents / dsh 模式。线升到 **v0.1.7**。
- Agent：`delete_instance` / `delete_mod`（工具卡 + skill）；永久删除需说清目标，多命中模组会拒删。
- （历史）线升到 v0.1.6；Windows Release 跟打。

## 日更备忘（2026-10-29）

- Agent：`rename_instance` / `duplicate_instance`（工具卡 + skill）；`open_game_folder` 扩到 config / logs / crash-reports。
- 线升到 v0.1.5；Windows Release 跟打。
