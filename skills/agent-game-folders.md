---
name: Agent 打开游戏文件夹
description: 当玩家要打开当前版本的模组、资源包、光影、存档、配置、日志或崩溃报告文件夹时使用。
---

# Agent：打开游戏文件夹

## 何时用

- 「打开模组文件夹」「mods 文件夹」
- 「打开资源包」「材质包文件夹」
- 「打开光影」「光影包文件夹」
- 「打开存档」「世界文件夹」
- 「打开配置文件夹」「config」
- 「打开日志文件夹」（整夹，不是单文件 latest.log）
- 「打开崩溃文件夹」「crash-reports」

## 步骤

1. 识别文件夹类型：`mods` / `resourcepacks` / `shaderpacks` / `saves` / `config` / `logs` / `crash-reports`。
2. 调用对应工具（`open_mods` 等 / `open_game_folder`），定位当前版本下的子目录；没有就创建空文件夹。
3. 工具卡展示路径与大约项数，并给「打开 xxx 文件夹」按钮（reveal）。

## 注意

- 「打开日志」若指单文件 `latest.log` / 崩溃报告文件，走 `open_latest_log`，不要和「打开日志文件夹」混淆。
- 中文玩家向文案；不要提 MCBBS。
- 只打开文件夹，不删文件、不改模组开关（禁用/启用仍用 toggle_mod）。
- 「打开版本文件夹」仍走整包游戏目录；本 skill 只覆盖上述子目录。
