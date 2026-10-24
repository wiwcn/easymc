# EasyMC 每日工作规划

时区：Asia/Shanghai。推送只用本机 `~/.config/easymc/gh_token`。不主动打扰 William（除非被挡住）。

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

UI 视觉仍走 EasyMC 自有 MD3 / Impeccable，不抄参照的皮肤。

## 固定节奏

| 时间 | 事项 |
|------|------|
| 工作日约 10:00 | 例行开发：拉最新 → 对照参照/缺口清单推进 → Agent 工具/skill → 有实质改动则打 Windows 包 |
| 每周至少一次 | GitHub 再搜 JE 启动器，更新「可借鉴」与缺口 |
| 按需 | 定稿后更新 Release（当前线：v0.1.1+） |

## 每日清单（按优先级）

1. **对齐**：`git pull` main，看其他协作者昨夜改了什么。
2. **功能对齐**：对照 PCL / HMCL / SJMCL（及本周搜到的项目）推进缺口；启动页偏 HMCL，其余偏 PCL；SJMCL 重点学跨平台 IA 与 Agent/扩展协作。
3. **Agent**：更多 MC 真工具 + 工具卡；稳定流程沉淀为 skill。
4. **整合包**：保持 `.mrpack` 导入/导出可用。
5. **打包**：当日有用户可见/可发布提交 → Windows NSIS + 更新 GitHub Release。
6. **收尾**：本对话短报进度与明日重点；无事则安静。

## 本周主题（滚动）

- P0：PCL / HMCL / SJMCL 缺失功能清单 → 逐项落地
- P0：Agent 工具面与 skill 库
- P1：`.mrpack` / 分享链路打磨
- P1：Windows 定稿包常绿
- P2：皮肤站、Android 真启动（桌面优先时后置；FCL 作能力参照）

## 不做

- 不写 MCBBS
- 不用 CloudAgent / Cursor GitHub MCP 改本仓
- 不把密钥写进 git
- 不把参照启动器的 UI 皮直接搬过来
