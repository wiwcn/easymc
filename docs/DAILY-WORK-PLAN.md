# EasyMC 每日工作规划

时区：Asia/Shanghai。推送只用本机 `~/.config/easymc/gh_token`。不主动打扰 William（除非被挡住）。

## 固定节奏

| 时间 | 事项 |
|------|------|
| 工作日约 10:00 | 例行开发：拉最新 → PCL/HMCL 对齐切片 → Agent 工具/skill → 有实质改动则打 Windows 包 |
| 按需 | 定稿后 `workflow_dispatch` 更新 Release（当前线：v0.1.1） |

## 每日清单（按优先级）

1. **对齐**：`git pull` main，看其他协作者昨夜改了什么。
2. **功能对齐 PCL / HMCL**：按缺口推进（启动/版本、下载六 Tab、账户、资源/模组、日志、联机预留等），不偷工减料；启动页逻辑偏 HMCL，其余偏 PCL；UI 走 Impeccable。
3. **Agent**：更多 MC 真工具 + 工具卡；稳定流程沉淀为 skill。
4. **整合包**：保持 `.mrpack` 导入/导出可用。
5. **打包**：当日有用户可见/可发布提交 → Windows NSIS + 更新 GitHub Release。
6. **收尾**：本对话短报进度与明日重点；无事则安静。

## 本周主题（滚动）

- P0：PCL/HMCL 缺失功能清单 → 逐项落地
- P0：Agent 工具面与 skill 库
- P1：`.mrpack` / 分享链路打磨
- P1：Windows 定稿包常绿
- P2：皮肤站、Android 真启动（桌面优先时后置）

## 不做

- 不写 MCBBS
- 不用 CloudAgent / Cursor GitHub MCP 改本仓
- 不把密钥写进 git
