# Minecraft JE 启动器扫描（2026-11-01，Asia/Shanghai）

对照真实 GitHub README / 仓库元数据（浅克隆于 `/tmp/mc-launchers-scan/`），不抄品牌与专有代码。EasyMC 许可：LGPL-3.0-or-later。

## 已调研启动器一览

| 启动器 | Stars（约） | 许可 | 栈 | 要点（功能/设计） |
|--------|------------|------|----|-------------------|
| PrismLauncher | ~11.0k | GPL-3.0 | C++/Qt6 | 多实例、组件层、Modrinth/CF、导出导入、日志/崩溃、快捷方式、Quick Play |
| HMCL | ~10.8k | GPL-3.0 | Java/JavaFX | 启动中心、自动装加载器、整合包、多架构跨平台、皮肤/自定义 UI |
| PCL（Meloong） | ~9.5k | 自定义 | C#/.NET | 下载/账户/功能面极全；源码有延迟同步；**勿抄品牌与独特文案** |
| PCL-CE | ~5.0k | Apache-2.0 + 自定义 UI | C# | 社区增强版 PCL 功能面 |
| FoldCraftLauncher (FCL) | ~5.1k | GPL-3.0 | Java/Android | 移动端 JE；多 Java、触控映射、渲染器插件；桌面后置参考 |
| MultiMC | ~4.7k | 专有条款 | C++ | 多实例鼻祖；Prism/PolyMC 血脉 |
| PolyMC | ~2.1k | GPL-3.0 | C++/Qt | Prism 前身血脉 |
| XMCL (x-minecraft-launcher) | ~1.7k | MIT | Electron/TS | 硬链资源去重、CF+Modrinth、多账户（含 LittleSkin）、P2P 联机 |
| GDLauncher | ~1.2k | GPL-3.0 | JS | 归档；强调 UX；Carbon 继任 |
| HeliosLauncher | ~1.0k | MIT | Electron | 定制服/整合包分发向 |
| PandoraLauncher | ~0.95k | MIT | Rust | 跨实例文件同步、**无上限实时日志**、CF+Modrinth、密钥环账户 |
| FreesmLauncher | ~0.91k | GPL-3.0 | C++ | Prism fork |
| ATLauncher | ~0.86k | GPL-3.0 | Java | 整合包一站式 |
| SJMCL | ~0.78k | GPL-3.0 | Tauri2+Next | **同栈**；实例中心、CF+Modrinth、deeplink/MCP/扩展/Agent |
| Axolotl | ~0.78k | Other | Rust | 跨平台「下一代」 |
| FluentLauncher | ~0.42k | MIT | C#/WinUI | Win11 Fluent；自动 Java；CF+Modrinth |
| Millida | ~0.44k | GPL-3.0 | Tauri2+React | 衣柜/好友/自建服面板；同属 Tauri 系 |
| QuantumLauncher | ~0.20k | GPL-3.0 | Rust | 轻量模组店、老版本/Omniarchive |
| scl | ~0.19k | **LGPL-3.0** | Rust | 核心库：启动/下载/微软登录/模组；主题等未完成 |
| Polymerium | ~0.13k | MIT | C#/.NET10 | **元数据驱动实例**、链接去重、原地换加载器、MCP/CLI |
| portablemc | ~0.56k | Apache-2.0 | Rust CLI | 可靠 CLI 启动+加载器 |
| CMCL（console-minecraft-launcher） | ~0.21k | GPL-3.0 | Java CLI | 正版/外置登录、模组搜索、快捷命令别名 |
| BakaXL | ~0.55k（门户） | 非全开源 | — | 功能面参考（举报开关等）；源码不全 |
| LauncherX (Corona) | 议题仓 | — | — | 游玩时长等能力见社区讨论；主仓未完整开源 |
| ManyMC / olauncher / BMCL / Nitrolaunch / OneLauncher / basalt / shard 等 | 不等 | 多为 GPL/MIT | 杂 | 血脉 fork、老启动器复活、Rust 新锐、内容寻址模组库等 |

> CMCL 名称常指 `MrShieh-X/console-minecraft-launcher`（命令行）。独立「CMCL」桌面产品未在公开 GitHub 找到活跃主仓。

## EasyMC 已有（相对同辈）

- 多实例 / 版本中心；Fabric/Quilt/Forge/NeoForge；BMCLAPI+测速；Modrinth+CF；mrpack/分享包
- 离线 + 微软 Device Code；皮肤衣柜（3D）；Quick Play；mclo.gs；Agent 诊断
- 启动页日志缓冲（`launch-log` / `get_logs`）；Java 检测与首选；容器/Podman（可选）
- 桌面 / 开始菜单快捷方式（0.1.6）；累计游玩时长 `total_play_secs`（0.1.6）
- 启动页实时日志控制台：过滤 / 复制 / mclo.gs（0.1.7 打磨）

## 缺口（相对扫描 + 既有 backlog）

| 优先级 | 缺口 | 参照 | 本会话 |
|--------|------|------|--------|
| P1 | ~~桌面 / 开始菜单快捷方式（可带 Quick Play）~~ | Prism / MultiMC / CMCL 别名 | **已实现（0.1.6）** |
| P1 | 第三方 Authlib / 统一通行证完整登录 | XMCL / CMCL / SJMCL | 延后 |
| P2 | ~~游玩时长统计~~ | LauncherX / HMCL 类 / Fluent | **已实现（0.1.6）** |
| P2 | ~~实时日志控制台打磨（过滤/复制/丢 mclo.gs）~~ | Pandora / Prism | **本会话实现（0.1.7）** |
| P2 | 批量模组更新 / 元数据驱动换 loader | Polymerium / XMCL | 延后 |
| P3 | 资源硬链去重 | XMCL / Polymerium / shard | 延后 |
| P3 | 扩展系统 / MCP 深度 | SJMCL | 部分有 Agent，扩展商店延后 |
| P3 | 主题/强调色自由 | FCL / HMCL | 延后 |
| — | Android 真启动 | FCL | 另仓，本任务不做 |

## 本会话选型理由

1. **桌面快捷方式**：backlog P1；已有启动脚本生成，差「放到桌面/应用菜单」这一步，玩家立刻可感。
2. **游玩时长**：peers 普遍有；仅需在进程生命周期记账 + 首页展示，契合现有 `ProcessManager`。

## 0.1.7 变更（实时控制台）

启动页「实时日志」：关键词/级别过滤、复制全部或选中、一键上传进程缓冲到 mclo.gs（空缓冲回退 latest.log）、玻璃卡片空态与跟随滚动。
