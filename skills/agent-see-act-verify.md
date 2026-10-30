---
name: Agent see→act→verify 崩溃排查
description: 当玩家启动失败、闪退、卡死或要分析崩溃/日志时使用；多步工具：先看→再做→再验证。
---

# Agent：see → act → verify（崩溃排查）

## 何时用

- 「崩溃」「闪退」「打不开」「启动不了」「卡死」
- 「查看崩溃」「分析日志」「修好 Java / 内存」

## 步骤

1. **See（先看）**：`analyze_crash_log` / `diagnose`；必要时 `open_latest_log`、`list_installed_mods` / `check_mods`。
2. **Act（再做）**：用「优化配置」/`propose_changes` 给出可逆建议；删除/禁用模组或删版本等危险操作只说明，等玩家明确确认。
3. **Verify（再验）**：改完后再诊断一次，或请玩家重启后再「查看崩溃」，对照摘要是否好转。

## 注意

- 玩家向中文；一次只推进一步。
- 危险工具不会在云端循环里自动执行。
- 不要提 MCBBS；不要编造启动器命令。
