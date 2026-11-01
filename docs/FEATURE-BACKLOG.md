# EasyMC 功能缺口 / 待办（玩家向）

对照小众与主流 JE 启动器的**能力**（Prism / PolyMC forks / MultiMC / CMCL / BakaXL / LauncherX / SCL / FCL 桌面侧思路 / Gradlew-launcher 等），只记产品合拍、且不抄品牌与文案的项。  
皮肤 3D / 衣柜、模组依赖图、模组·光影图标多列等由并行任务推进，此处不重复实现说明。

## 已落地（本轮）

| 功能 | 参照灵感 | 说明 |
|------|----------|------|
| **快速进入（Quick Play）** | CMCL、Prism `--quickPlay*` | 版本设置可选世界 / 服务器 / Realms；世界页「设为快速进入」「直进」。1.20+ 用官方参数；更旧版本服务器回退 `--server`/`--port`。 |
| **分享日志（mclo.gs）** | 多数启动器的日志上传 | 版本设置「分享最新日志」→ 上传 `latest.log`（或最新 crash-report）到 mclo.gs，复制链接。 |

## 高价值待做（未实现）

| 优先级 | 功能 | 参照灵感 | 备注 |
|--------|------|----------|------|
| P1 | ~~桌面 / 开始菜单快捷方式~~ | Prism、MultiMC | **已实现（0.1.6）**：`create_desktop_shortcut` → 桌面 + 开始菜单 / applications；脚本含 Quick Play。 |
| P1 | **禁用聊天举报（可选）** | BakaXL、部分 Prism 配置 | 以玩家可理解的开关呈现；实现需跟进当年 MC/加载器可行方式（JVM/模组），避免静默改协议。 |
| P2 | ~~游玩时长统计~~ | LauncherX、HMCL 类 | **已实现（0.1.6）**：`total_play_secs` 会话累计；启动页 / 版本设置展示。 |
| P2 | **启动后实时日志控制台** | Prism、MultiMC | 游戏进程 stdout/崩溃尾部可在启动器内查看，并一键丢给 mclo.gs。 |
| P3 | **便携模式 / 绿色目录** | Gradlew-launcher、部分 SCL 用法 | 数据目录跟随安装路径，便于 U 盘或免安装分发。 |
| P3 | **中间件 / 组件列表（高级）** | MultiMC / Prism components | 对 loader、LWJGL 等可替换层做可视化；偏高手，排在基础体验之后。 |

## 明确不在本清单

- 抄袭其它启动器的品牌、图标、独特文案或整页视觉皮。
- 与并行中的皮肤预览、依赖树、资源图标网格重复的条目（见每日计划与对应 PR）。
