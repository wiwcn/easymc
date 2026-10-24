---
name: Agent 模组开关与整合包
description: 当玩家要启用/禁用模组、看版本列表，或导入/导出 Modrinth .mrpack 时使用。
---

# Agent：模组开关与 .mrpack

## 何时用

- 玩家说「禁用 sodium」「启用 iris」「切换模组 xxx」
- 「版本列表」「有哪些版本」
- 「制作整合包」导出当前版本 `.mrpack`
- 「导入整合包 /path/to/pack.mrpack」把本地包装成新版本

## 步骤

1. 确认当前选中版本（启动页 / 助手顶栏）；未选则先让玩家选。
2. **禁用/启用**：本地改名 `mods/*.jar` ↔ `*.jar.disabled`；多匹配时列出文件让玩家说具体名。
3. **版本列表**：调用 `list_instances`，★ 标记当前版本。
4. **导出**：`create_modpack` / 「制作整合包」→ 写出 `.mrpack`（CDN + overrides），工具卡给「打开整合包位置」。
5. **导入**：路径必须是本机可读的 `.mrpack` / 含 `modrinth.index.json` 的 zip / EasyMC 分享包；导入后是**新版本**，不覆盖旧的。
6. 工具结果一律走完整工具卡（标题、参数、状态、按钮），不要只回纯文字。

## 注意

- 中文玩家向文案；不要提 MCBBS。
- 不静默写设置：内存/JVM 走「建议更改」卡片。
- 桌面优先；Android 真启动另线。
