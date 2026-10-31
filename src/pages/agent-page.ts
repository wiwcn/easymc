import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentApplyProposal,
  agentAsk,
  agentBumpMemory,
  agentDataDir,
  agentExportDiagnostics,
  agentLlmConfigClear,
  agentLlmConfigGet,
  agentLlmConfigSave,
  agentLlmStatus,
  agentUndoProposal,
  listInstancesState,
  openInstanceFolder,
} from "../api";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import "../components/empty-state";
import type {
  AgentActionDto,
  AgentAskResponse,
  AgentFinding,
  ChangeProposalDto,
  DiagnoseReportDto,
  InstanceDto,
  LlmConfigViewDto,
  LlmStatusDto,
  ToolCallDto,
  ToolStatus,
} from "../types";

type ChatTurn = {
  role: "user" | "assistant";
  content: string;
  mode?: string;
  proposal?: ChangeProposalDto | null;
  /** applied | undone | pending */
  proposalStatus?: "pending" | "applied" | "undone";
  /** Primary / last tool card (compat with single-tool turns). */
  tool?: ToolCallDto | null;
  /** All tool cards from a multi-step agent loop. */
  tools?: ToolCallDto[];
};


const TOOL_TITLES: Record<string, string> = {
  ensure_java: "准备 Java",
  setup_env: "一键环境",
  install_mod: "安装模组",
  search_mods: "搜索模组",
  suggest_mods: "推荐模组",
  open_store: "打开下载",
  install_mod_hint: "打开下载",
  bump_memory: "调整内存",
  propose_changes: "建议更改",
  set_download_source: "切换下载源",
  open_instance_dir: "打开版本文件夹",
  show_data_dir: "数据目录",
  check_mods: "已装模组",
  list_installed_mods: "已装模组",
  toggle_mod: "启用/禁用模组",
  enable_mod: "启用模组",
  disable_mod: "禁用模组",
  list_instances: "版本列表",
  list_versions: "版本列表",
  select_instance: "切换版本",
  switch_instance: "切换版本",
  switch_version: "切换版本",
  rename_instance: "重命名版本",
  rename_version: "重命名版本",
  duplicate_instance: "复制版本",
  duplicate_version: "复制版本",
  copy_instance: "复制版本",
  open_latest_log: "打开日志",
  open_log: "打开日志",
  open_game_folder: "打开游戏文件夹",
  open_mods: "打开模组文件夹",
  open_resourcepacks: "打开资源包",
  open_shaderpacks: "打开光影",
  open_saves: "打开存档",
  open_config: "打开配置文件夹",
  open_logs: "打开日志文件夹",
  "open_crash-reports": "打开崩溃文件夹",
  open_crash_reports: "打开崩溃文件夹",
  open_screenshots: "打开截图",
  select_account: "切换账户",
  switch_account: "切换账户",
  list_accounts: "账户列表",
  import_mrpack: "导入整合包",
  import_modpack: "导入整合包",
  build_pack: "制作整合包",
  create_modpack: "制作整合包",
  export_modpack: "制作整合包",
  diagnostics: "导出问题报告",
  delete_instance: "删除版本",
  delete_version: "删除版本",
  remove_instance: "删除版本",
  delete_mod: "删除模组",
  remove_mod: "删除模组",
  uninstall_mod: "删除模组",
  diagnose: "查看崩溃",
  analyze_crash_log: "查看崩溃",
  analyze_launch_log: "查看崩溃",
  suggest_memory: "内存建议",
};

/** Player-facing skill hints (mirror crates/easymc-agent skills index). */
const SKILL_HINTS: { label: string; prompt: string; tool?: string }[] = [
  { label: "游戏崩了", prompt: "查看崩溃", tool: "diagnose" },
  { label: "装个模组", prompt: "搜模组 sodium", tool: "search_mods" },
  { label: "打开模组文件夹", prompt: "打开模组文件夹", tool: "open_mods" },
  { label: "看看有哪些版本", prompt: "版本列表", tool: "list_instances" },
  { label: "内存不够卡", prompt: "优化配置", tool: "propose_changes" },
  { label: "禁用某个模组", prompt: "已装模组", tool: "check_mods" },
];

function runningTool(name: string, args: { key: string; label_zh: string; value: string }[] = []): ToolCallDto {
  return {
    id: `local-running-${name}-${Date.now()}`,
    name,
    title_zh: TOOL_TITLES[name] || "工具",
    status: "running",
    args,
    result_zh: "正在执行…",
    path: null,
    payload: null,
    ui_actions: [],
    tag_zh: null,
  };
}

function asTool(value: unknown): ToolCallDto | null {
  if (!value || typeof value !== "object") return null;
  const v = value as ToolCallDto;
  if (!v.id || !v.name || !v.title_zh || !v.status) return null;
  return {
    ...v,
    args: Array.isArray(v.args) ? v.args : [],
    ui_actions: Array.isArray(v.ui_actions) ? v.ui_actions : [],
  };
}

function statusLabel(s: ToolStatus): string {
  switch (s) {
    case "pending":
      return "待确认";
    case "running":
      return "进行中";
    case "ok":
      return "完成";
    case "error":
      return "没成功";
    default:
      return s;
  }
}

function asProposal(value: unknown): ChangeProposalDto | null {
  if (!value || typeof value !== "object") return null;
  const v = value as ChangeProposalDto;
  if (!v.id || !Array.isArray(v.items) || !v.apply_payload || !v.undo_snapshot) return null;
  return v;
}

@customElement("easymc-agent-page")
export class AgentPage extends LitElement {
  @state() private llm: LlmStatusDto | null = null;
  @state() private report: DiagnoseReportDto | null = null;
  @state() private busy = false;
  @state() private input = "";
  @state() private turns: ChatTurn[] = [];
  @state() private instances: InstanceDto[] = [];
  @state() private instanceId: string | null = null;
  @state() private error = "";
  /** Failed install → offer one-tap jump to download search. */
  @state() private pendingStoreQuery: string | null = null;
  @state() private showLlmForm = false;
  @state() private llmCfg: LlmConfigViewDto | null = null;
  @state() private llmBase = "";
  @state() private llmKey = "";
  @state() private llmModel = "";
  @state() private llmBusy = false;
  @state() private booting = true;

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    :host {
      display: block;
      /* Column width comes from the shell (.page > * in app-shell.ts).
         Fill the page box instead of guessing chrome height with 100vh. */
      height: 100%;
      min-height: 480px;
    }
    .layout {
      display: grid;
      grid-template-columns: minmax(0, 1fr) 260px;
      gap: var(--easymc-space-4);
      height: 100%;
      min-height: 0;
    }
    .main-col {
      display: flex;
      flex-direction: column;
      min-height: 0;
      min-width: 0;
    }
    .head {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--easymc-space-3);
      margin-bottom: var(--easymc-space-3);
    }
    h1 {
      margin: 0;
      display: flex;
      align-items: center;
      gap: var(--easymc-space-3);
      flex-wrap: wrap;
    }
    .sub {
      margin: var(--easymc-space-1) 0 0;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: var(--easymc-space-2);
      padding: 4px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
    }
    .badge.on {
      background: var(--md-sys-color-success-container);
      color: var(--md-sys-color-on-success-container);
    }
    button,
    .chip {
      min-height: 32px;
      padding: 8px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      font: inherit;
      font-size: var(--easymc-type-label);
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.primary { border-color: transparent; }
    button:disabled { opacity: 0.5; cursor: not-allowed; }
    button:active:not(:disabled), .chip:active { transform: scale(0.98); }
    button:focus-visible, .chip:focus-visible {
      outline: none;
      box-shadow: var(--easymc-focus-ring);
    }
    select {
      padding: 8px 12px;
      border-radius: var(--md-sys-shape-corner-small);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
    }

    .gate-shell {
      display: flex;
      flex-direction: column;
      height: 100%;
      min-height: 0;
    }
    .gate-shell > .head {
      flex-shrink: 0;
    }
    .gate-body {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 0;
      padding: var(--easymc-space-3) 0 var(--easymc-space-5);
    }
    .boot-muted {
      margin: 48px auto;
      text-align: center;
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-body);
    }
    .chat-empty {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 240px;
      margin-bottom: var(--easymc-space-3);
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      box-sizing: border-box;
    }
    .skills-hints {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
      margin-bottom: var(--easymc-space-3);
    }
    .skills-hints .hint-label {
      margin: 0;
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface-variant);
    }
    .skills-hints .hint-row {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
    }
    .tool-card.pending {
      outline-color: color-mix(in oklch, var(--md-sys-color-tertiary, var(--md-sys-color-secondary)) 45%, transparent);
    }
    .tool-result.muted-fallback {
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta);
    }
    .chat {
      flex: 1;
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-3);
      overflow: auto;
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      margin-bottom: var(--easymc-space-3);
      min-height: 240px;
    }
    .msg {
      display: flex;
      gap: var(--easymc-space-3);
      align-items: flex-start;
      max-width: 96%;
    }
    .msg.user { align-self: flex-end; flex-direction: row-reverse; }
    .msg.assistant { align-self: flex-start; width: min(100%, 640px); }
    .avatar {
      width: 32px;
      height: 32px;
      border-radius: 50%;
      flex-shrink: 0;
      display: grid;
      place-items: center;
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-bold);
    }
    .msg.assistant .avatar {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
    }
    .msg.user .avatar {
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
    }
    .bubble {
      padding: 11px 15px;
      border-radius: var(--md-sys-shape-corner-large);
      white-space: pre-wrap;
      font-size: var(--easymc-type-body);
      line-height: 1.45;
    }
    .msg.user .bubble {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      border-bottom-right-radius: 6px;
    }
    .msg.assistant .bubble {
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
      border-bottom-left-radius: 6px;
    }
    .meta {
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      margin-top: 4px;
    }

    /* Normalized tool card */
    .tool-card {
      margin-top: var(--easymc-space-3);
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      border: none;
      outline: 1px solid transparent;
    }
    .tool-card.running {
      outline-color: color-mix(in oklch, var(--md-sys-color-primary) 45%, transparent);
      box-shadow: var(--md-sys-elevation-2);
    }
    .tool-card.error {
      outline-color: color-mix(in oklch, var(--md-sys-color-error) 55%, transparent);
    }
    .tool-card-head {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--easymc-space-3);
      margin-bottom: var(--easymc-space-3);
    }
    .tool-card-head h3 {
      margin: 0;
      font-size: var(--easymc-type-title);
      font-weight: var(--easymc-weight-medium);
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .tool-chip {
      flex-shrink: 0;
      padding: 3px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
    }
    .tool-chip.ok {
      background: var(--md-sys-color-success-container, var(--md-sys-color-primary-container));
      color: var(--md-sys-color-on-success-container, var(--md-sys-color-on-primary-container));
    }
    .tool-chip.running {
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
    }
    .tool-chip.error {
      background: var(--md-sys-color-error-container, #fce8e6);
      color: var(--md-sys-color-on-error-container, var(--md-sys-color-error));
    }
    .tool-chip.pending {
      background: var(--md-sys-color-tertiary-container, var(--md-sys-color-secondary-container));
      color: var(--md-sys-color-on-tertiary-container, var(--md-sys-color-on-secondary-container));
    }
    .tool-args {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: 6px 12px;
      margin-bottom: var(--easymc-space-3);
      font-size: var(--easymc-type-label);
    }
    .tool-args dt {
      margin: 0;
      color: var(--md-sys-color-on-surface-variant);
      font-weight: var(--easymc-weight-medium);
      font-size: var(--easymc-type-meta);
    }
    .tool-args dd {
      margin: 0;
      word-break: break-word;
    }
    .tool-result {
      margin: 0 0 var(--easymc-space-3);
      font-size: var(--easymc-type-body);
      line-height: 1.45;
      white-space: pre-wrap;
      color: var(--md-sys-color-on-surface);
    }
    .tool-card-actions {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
    }
    .tool-progress {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: var(--easymc-space-2);
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .tool-progress .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--md-sys-color-primary);
      animation: tool-pulse 1s ease-in-out infinite;
    }
    @keyframes tool-pulse {
      0%, 100% { opacity: 0.35; transform: scale(0.9); }
      50% { opacity: 1; transform: scale(1); }
    }
    @media (prefers-reduced-motion: reduce) {
      .tool-progress .dot { animation: none; opacity: 1; }
    }
    /* Proposal card (ghost card toward agent-v1) */
    .proposal {
      margin-top: var(--easymc-space-3);
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      border: none;
    }
    .proposal-head {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: var(--easymc-space-3);
      margin-bottom: var(--easymc-space-3);
    }
    .proposal-head h3 {
      margin: 0;
      font-size: var(--easymc-type-title);
      font-weight: var(--easymc-weight-medium);
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .proposal-sub {
      margin: 4px 0 0;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.35;
    }
    .tag {
      flex-shrink: 0;
      padding: 3px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
    }
    .proposal-table {
      width: 100%;
      border-collapse: collapse;
      font-size: var(--easymc-type-label);
      margin-bottom: var(--easymc-space-3);
    }
    .proposal-table th,
    .proposal-table td {
      text-align: left;
      padding: 8px 6px;
      border-bottom: 1px solid color-mix(in srgb, var(--md-sys-color-outline-variant) 40%, transparent);
      vertical-align: top;
    }
    .proposal-table th {
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta);
    }
    .proposal-table .delta {
      color: var(--md-sys-color-primary);
      font-weight: var(--easymc-weight-medium);
    }
    .proposal-table .proposed {
      font-weight: var(--easymc-weight-medium);
    }
    .proposal-actions {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
    }
    .proposal-actions .ghost {
      background: transparent;
      border: 1px solid var(--md-sys-color-outline-variant);
    }
    .proposal-status {
      margin-top: var(--easymc-space-2);
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .proposal-status.ok {
      color: var(--md-sys-color-primary);
      font-weight: var(--easymc-weight-medium);
    }
    .composer {
      display: flex;
      gap: var(--easymc-space-2);
      align-items: center;
    }
    .composer input {
      flex: 1;
      padding: 13px 16px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
    }
    .tools {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      border: none;
      align-self: start;
      max-height: 100%;
      overflow: auto;
    }
    .tools h2 {
      margin: 0 0 var(--easymc-space-2);
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface-variant);
      letter-spacing: 0.02em;
      text-transform: none;
    }
    .tool-stack {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
    }
    button.tool {
      display: grid;
      grid-template-columns: 1fr auto;
      align-items: center;
      gap: var(--easymc-space-2);
      text-align: left;
      width: 100%;
      padding: var(--easymc-space-3) var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      border: none;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      min-height: 56px;
      /* MI-5: hover lifts elevation 1 → 2 (dur-fast), no spring overshoot. */
      transition:
        background var(--easymc-dur-fast) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-fast) var(--easymc-ease-out);
    }
    button.tool .tool-copy {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      gap: 2px;
      min-width: 0;
    }
    button.tool strong {
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface);
    }
    button.tool span {
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.3;
    }
    button.tool .chev {
      color: var(--md-sys-color-outline);
      font-size: 1.1em;
      line-height: 1;
    }
    button.tool:hover {
      background: var(--md-sys-color-primary-container);
      box-shadow: var(--md-sys-elevation-2);
    }
    button.tool:hover strong {
      color: var(--md-sys-color-on-primary-container);
    }
    button.tool:hover span {
      color: var(--md-sys-color-on-primary-container);
      opacity: 0.85;
    }
    .tools .note {
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      margin-top: var(--easymc-space-2);
      line-height: 1.35;
    }
    .quick-chips {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
      margin-bottom: var(--easymc-space-3);
    }
    /* Queried against the page column, so the sidebar drops out when the page
       itself gets narrow — not merely when the window does. */
    @container easymc-page (max-width: 820px) {
      .layout { grid-template-columns: minmax(0, 1fr); }
      .tools { display: none; }
    }
    .findings {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-3);
      margin: 0 0 12px;
    }
    .card {
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
    }
    .card.error { outline: 1px solid var(--md-sys-color-error); }
    .card.warn { outline: 1px solid var(--md-sys-color-warning); }
    .card h3 { margin: 0 0 6px; font-size: var(--easymc-type-title); }
    .card p { margin: 0 0 6px; font-size: var(--easymc-type-body); color: var(--md-sys-color-on-surface-variant); }
    .card ul { margin: 0; padding-left: 1.2em; font-size: var(--easymc-type-label); }
    .evidence {
      font-family: ui-monospace, monospace;
      font-size: var(--easymc-type-meta);
      opacity: 0.85;
      word-break: break-all;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
      margin-bottom: 10px;
      align-items: center;
    }
    .err { color: var(--md-sys-color-error); margin: 8px 0; }
    @media (prefers-reduced-motion: reduce) {
      button:active:not(:disabled), .chip:active { transform: none; }
    }
    .gate {
      max-width: 520px;
      width: 100%;
      margin: 24px auto;
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      box-sizing: border-box;
    }
    .gate h2 {
      margin: 0 0 8px;
      font-size: var(--easymc-type-title);
    }
    .gate .muted {
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta);
      margin: 0 0 14px;
      line-height: 1.45;
    }
    .gate label.row {
      display: grid;
      grid-template-columns: 4.5em minmax(0, 1fr);
      gap: var(--easymc-space-3);
      align-items: center;
      width: 100%;
      margin-bottom: var(--easymc-space-3);
    }
    .gate label.row span {
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      white-space: nowrap;
    }
    .gate input {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 8px 14px;
      font: inherit;
      min-width: 0;
      flex: 1;
    }
  `,
  ];

  connectedCallback() {
    super.connectedCallback();
    void this.boot();
  }

  private async boot() {
    this.booting = true;
    try {
      const [st, cfg, inst] = await Promise.all([
        agentLlmStatus(),
        agentLlmConfigGet(),
        listInstancesState(),
      ]);
      this.llm = st;
      this.llmCfg = cfg;
      this.llmBase = cfg.base_url || "";
      this.llmModel = cfg.model || "deepseek-chat";
      this.llmKey = "";
      // Cloud-first: unconfigured → blocking gate (no local chat by default).
      this.showLlmForm = !st.configured;
      this.instances = inst.instances;
      this.instanceId = inst.active_id;
      if (st.configured) {
        // Structured empty chat teaches next steps (Impeccable onboard / Extremes).
        this.turns = [];
        if (sessionStorage.getItem("easymc-agent-autodiagnose") === "1") {
          sessionStorage.removeItem("easymc-agent-autodiagnose");
          void this.onDiagnose();
        }
      } else {
        sessionStorage.removeItem("easymc-agent-autodiagnose");
        this.turns = [];
        this.report = null;
      }
    } catch (e) {
      this.error = friendlyError(e);
    } finally {
      this.booting = false;
    }
  }

  private get cloudReady(): boolean {
    return !!this.llm?.configured;
  }

  private requireCloud(): boolean {
    if (this.cloudReady) return true;
    this.showLlmForm = true;
    this.error = "请先填好助手的接口地址和密钥，才能开始聊天哦。";
    return false;
  }

  private async onDiagnose() {
    await this.onAsk("查看崩溃", runningTool("diagnose"));
  }

  private async onSearchModsPrompt() {
    if (!this.requireCloud()) return;
    const q = window.prompt("搜什么模组？（例如 sodium / create / jei）", "sodium");
    if (!q || !q.trim()) return;
    const query = q.trim();
    await this.onAsk(
      `搜模组 ${query}`,
      runningTool("search_mods", [{ key: "query", label_zh: "关键词", value: query }]),
    );
  }

  private async onSuggestMods() {
    await this.onAsk(
      "推荐模组 生存",
      runningTool("suggest_mods", [{ key: "theme", label_zh: "主题", value: "生存" }]),
    );
  }

  private async onAsk(text?: string, runningHint?: ToolCallDto) {
    if (!this.requireCloud()) return;
    const q = (text ?? this.input).trim();
    if (!q || this.busy) return;
    this.input = "";
    this.busy = true;
    this.error = "";
    this.turns = [...this.turns, { role: "user", content: q }];
    // Optimistic running card so every tool path shows a complete card immediately.
    if (runningHint) {
      this.turns = [
        ...this.turns,
        {
          role: "assistant",
          content: "好的，我来处理。",
          mode: "running",
          tool: runningHint,
          tools: [runningHint],
        },
      ];
    }
    try {
      const history = this.turns
        .filter((t) => t.role === "user" || t.role === "assistant")
        .filter((t) => t.mode !== "running")
        .slice(-10)
        .map((t) => ({ role: t.role, content: t.content }));
      const resp: AgentAskResponse = await agentAsk(q, {
        instanceId: this.instanceId,
        history,
      });
      if (resp.report) this.report = resp.report;
      let content = resp.reply_zh;
      let proposal: ChangeProposalDto | null = null;
      let proposalStatus: ChatTurn["proposalStatus"];
      const toolsFromResp: ToolCallDto[] = (resp.tools ?? [])
        .map((x) => asTool(x))
        .filter((x): x is ToolCallDto => !!x);
      let tool = asTool(resp.tool)
        ?? toolsFromResp[toolsFromResp.length - 1]
        ?? (resp.action ? asTool({
        id: `from-action-${Date.now()}`,
        name: resp.action.action,
        title_zh: TOOL_TITLES[resp.action.action] || "工具",
        status: resp.action.ok ? "ok" : "error",
        args: [],
        result_zh: resp.action.message_zh,
        path: resp.action.path,
        payload: resp.action.value,
        ui_actions: [],
        tag_zh: null,
      }) : null);
      let tools: ToolCallDto[] = toolsFromResp.length
        ? toolsFromResp
        : tool
          ? [tool]
          : [];
      if (resp.action) {
        if (resp.action.action === "propose_changes") {
          proposal = asProposal(resp.action.value) ?? asProposal(tool?.payload);
          proposalStatus = proposal ? "pending" : undefined;
          if (tool && proposal) {
            tool = {
              ...tool,
              status: "pending",
              title_zh: proposal.title_zh || tool.title_zh,
              tag_zh: proposal.tag_zh || tool.tag_zh,
              payload: proposal,
              ui_actions: [
                { id: "apply_proposal", label_zh: "应用更改" },
                { id: "undo_proposal", label_zh: "撤销" },
              ],
            };
            if (tools.length) {
              tools = [...tools.slice(0, -1), tool];
            } else {
              tools = [tool];
            }
          }
        } else {
          content = this.enrichActionReply(content, resp.action);
          void this.applyAction(resp.action);
        }
      }
      // Prefer short bubble when the tool card already carries the result body.
      if (tool?.result_zh && content.trim() === tool.result_zh.trim()) {
        content = tool.status === "error" ? "这次没做成。" : "好了。";
      }
      if (tools.length === 0 && tool) tools = [tool];
      const assistantTurn: ChatTurn = {
        role: "assistant",
        content,
        mode: resp.mode,
        proposal,
        proposalStatus,
        tool,
        tools,
      };
      if (runningHint) {
        // Replace the optimistic running turn.
        this.turns = this.turns.map((t, i) =>
          i === this.turns.length - 1 && t.tool?.status === "running" ? assistantTurn : t,
        );
      } else {
        this.turns = [...this.turns, assistantTurn];
      }
    } catch (e) {
      this.error = friendlyError(e);
      const errTurn: ChatTurn = {
        role: "assistant",
        content: `出错了：${e}`,
        tool: runningHint
          ? { ...runningHint, status: "error", result_zh: friendlyError(e) }
          : null,
      };
      if (runningHint) {
        this.turns = this.turns.map((t, i) =>
          i === this.turns.length - 1 && t.tool?.status === "running" ? errTurn : t,
        );
      } else {
        this.turns = [...this.turns, errTurn];
      }
    } finally {
      this.busy = false;
    }
  }

  private enrichActionReply(reply: string, action: AgentActionDto): string {
    if (action.action === "bump_memory" && action.ok) {
      if (!reply.includes("重新启动")) {
        return `${reply}\n\n下一步：回到启动页，重新点「启动游戏」。`;
      }
    }
    if (action.action === "diagnostics" && action.ok && action.path) {
      if (!reply.includes(action.path)) {
        return `${reply}\n\n文件：${action.path}`;
      }
    }
    if (
      action.action === "ensure_java" ||
      action.action === "open_store" ||
      action.action === "install_mod" ||
      action.action === "setup_env" ||
      action.action === "build_pack" ||
      action.action === "set_download_source" ||
      action.action === "check_mods" ||
      action.action === "open_instance_dir"
    ) {
      return action.message_zh || reply;
    }
    return reply;
  }

  private goDownloadSearch(query: string, tab = "mod") {
    const q = query.trim();
    if (!q) return;
    sessionStorage.setItem("easymc-download-tab", tab);
    sessionStorage.setItem("easymc-download-query", q);
    this.pendingStoreQuery = null;
    showToast(`去下载页搜索「${q}」`);
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: "download", bubbles: true, composed: true }),
    );
  }

  /** Plain jump to download hub (no search query). */
  private goDownloadCenter() {
    this.pendingStoreQuery = null;
    sessionStorage.removeItem("easymc-download-query");
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: "download", bubbles: true, composed: true }),
    );
  }

  private async applyAction(action: AgentActionDto) {
    if (action.action === "install_mod") {
      this.pendingStoreQuery = null;
      showToast(action.ok ? "模组已安装" : "安装模组失败");
      return;
    }
    if (action.action === "open_store" || action.action === "install_mod_hint") {
      const val = action.value as { query?: string; tab?: string } | null;
      const query = val?.query?.trim() || "";
      const tab = val?.tab || "mod";
      if (!query) return;
      if (action.ok) {
        this.goDownloadSearch(query, tab);
      } else {
        this.pendingStoreQuery = query;
        showToast("安装没成功，可点下方去下载页");
      }
      return;
    }
    if (action.action === "ensure_java" || action.action === "setup_env") {
      showToast(action.ok ? "环境已准备好" : "准备环境失败");
      return;
    }
    if (action.action === "bump_memory" && action.ok) {
      showToast("内存已调整，请重新启动游戏");
      return;
    }
    if (action.action === "search_mods" || action.action === "suggest_mods") {
      showToast(action.ok ? (action.action === "search_mods" ? "模组搜索完成" : "推荐已就绪") : "这次没找到");
      return;
    }
    if (action.action === "build_pack") {
      showToast(action.ok ? "整合包已制作好（.mrpack）" : "制作整合包失败");
      return;
    }
    if (action.action === "set_download_source" && action.ok) {
      showToast("下载源已切换");
      return;
    }
    if (action.action === "open_instance_dir" && action.ok && action.path) {
      // Try native reveal; path already in chat.
      if (this.instanceId) {
        try {
          await openInstanceFolder(this.instanceId, "root");
        } catch {
          /* path shown in chat is enough */
        }
      }
      showToast("已定位版本文件夹");
      return;
    }
    if (action.action === "diagnostics" && action.ok) {
      showToast(action.message_zh.slice(0, 80));
    }
  }

  private patchTurnProposal(turnIndex: number, status: ChatTurn["proposalStatus"], extraContent?: string) {
    this.turns = this.turns.map((t, i) => {
      if (i !== turnIndex) return t;
      const toolStatus: ToolStatus =
        status === "applied" || status === "undone"
          ? "ok"
          : status === "pending"
            ? "pending"
            : (t.tool?.status ?? "ok");
      const patchOne = (tool: ToolCallDto): ToolCallDto => {
        let actions = tool.ui_actions || [];
        if (status === "applied") {
          actions = actions.filter((a) => a.id !== "apply_proposal");
          if (!actions.some((a) => a.id === "undo_proposal")) {
            actions = [...actions, { id: "undo_proposal", label_zh: "撤销" }];
          }
        } else if (status === "undone") {
          actions = actions.filter(
            (a) => a.id !== "apply_proposal" && a.id !== "undo_proposal",
          );
        }
        return {
          ...tool,
          status: toolStatus,
          result_zh: extraContent || tool.result_zh,
          ui_actions: actions,
        };
      };
      const tool = t.tool ? patchOne(t.tool) : t.tool;
      const tools = t.tools?.map((tc) =>
        tc.name === "propose_changes" || (t.tool && tc.id === t.tool.id)
          ? patchOne(tc)
          : tc,
      );
      return {
        ...t,
        proposalStatus: status,
        content: extraContent ? `${t.content}

${extraContent}` : t.content,
        tool,
        tools,
      };
    });
  }

  private async onAcceptProposal(turnIndex: number, proposal: ChangeProposalDto) {
    if (this.busy) return;
    this.busy = true;
    try {
      const r = await agentApplyProposal(proposal);
      const msg = r.message_zh || r.message || "已应用更改，下次启动生效";
      showToast("已应用更改，下次启动生效");
      this.patchTurnProposal(turnIndex, "applied", msg);
    } catch (e) {
      this.error = friendlyError(e);
      showToast("应用更改失败");
    } finally {
      this.busy = false;
    }
  }

  private async onUndoProposal(turnIndex: number, proposal: ChangeProposalDto) {
    if (this.busy) return;
    this.busy = true;
    try {
      const r = await agentUndoProposal(proposal);
      const msg = r.message_zh || r.message || "已撤销更改";
      showToast("已撤销更改");
      this.patchTurnProposal(turnIndex, "undone", msg);
    } catch (e) {
      this.error = friendlyError(e);
      showToast("撤销失败");
    } finally {
      this.busy = false;
    }
  }

  private async onEnsureJava() {
    await this.onAsk("修好 Java", runningTool("ensure_java"));
  }

  private async onSetupEnv() {
    await this.onAsk("一键环境", runningTool("setup_env"));
  }

  private async onTuneConfig() {
    await this.onAsk("优化配置", runningTool("propose_changes"));
  }

  private async onBuildPack() {
    await this.onAsk("制作整合包", runningTool("build_pack"));
  }

  private async onBmclapi() {
    await this.onAsk(
      "切换 BMCLAPI 加速",
      runningTool("set_download_source", [
        { key: "source", label_zh: "目标", value: "BMCLAPI" },
      ]),
    );
  }

  private async onOpenInstanceDir() {
    await this.onAsk("打开版本文件夹", runningTool("open_instance_dir"));
  }

  private async onCheckMods() {
    await this.onAsk("已装模组", runningTool("check_mods"));
  }

  private async onDataDir() {
    if (!this.requireCloud()) return;
    this.turns = [...this.turns, { role: "user", content: "游戏文件夹在哪" }];
    const r = await agentDataDir();
    showToast(r.message.slice(0, 60));
    const tool: ToolCallDto = {
      id: `data-dir-${Date.now()}`,
      name: "show_data_dir",
      title_zh: "数据目录",
      status: "ok",
      args: r.path ? [{ key: "path", label_zh: "路径", value: String(r.path) }] : [],
      result_zh: r.message,
      path: r.path ?? null,
      payload: null,
      ui_actions: [],
      tag_zh: "文件",
    };
    this.turns = [...this.turns, { role: "assistant", content: "在这儿。", mode: "rules", tool }];
  }

  private async onBumpMem() {
    if (!this.requireCloud()) return;
    this.busy = true;
    try {
      this.turns = [...this.turns, { role: "user", content: "内存调到 4 GB" }];
      const run = runningTool("bump_memory", [
        { key: "memory", label_zh: "目标内存", value: "4096 MB" },
      ]);
      this.turns = [
        ...this.turns,
        { role: "assistant", content: "好的，我来调内存。", mode: "running", tool: run },
      ];
      const r = await agentBumpMemory(4096);
      showToast(r.ok ? "内存已调整" : "调整失败");
      const tool: ToolCallDto = {
        ...run,
        status: r.ok ? "ok" : "error",
        result_zh: r.ok
          ? `${r.message}\n\n下一步：回到启动页，重新点「启动游戏」。`
          : r.message,
        payload: { memory_mb: 4096 },
      };
      const turn: ChatTurn = {
        role: "assistant",
        content: r.ok ? "内存已改好。" : "这次没调成。",
        mode: "rules",
        tool,
      };
      this.turns = this.turns.map((x, i) =>
        i === this.turns.length - 1 && x.tool?.status === "running" ? turn : x,
      );
    } catch (e) {
      this.error = friendlyError(e);
    } finally {
      this.busy = false;
    }
  }

  private async onExport() {
    if (!this.requireCloud()) return;
    this.busy = true;
    try {
      this.turns = [...this.turns, { role: "user", content: "导出问题报告" }];
      const run = runningTool("diagnostics");
      this.turns = [
        ...this.turns,
        { role: "assistant", content: "正在整理问题报告…", mode: "running", tool: run },
      ];
      const r = await agentExportDiagnostics(this.instanceId);
      showToast(r.ok ? "报告已导出" : "导出失败");
      const tool: ToolCallDto = {
        ...run,
        status: r.ok ? "ok" : "error",
        result_zh: r.message,
        path: r.path ?? null,
        args: r.path ? [{ key: "path", label_zh: "报告", value: String(r.path) }] : [],
      };
      const turn: ChatTurn = {
        role: "assistant",
        content: r.ok ? "报告已写好。" : "导出失败。",
        mode: "rules",
        tool,
      };
      this.turns = this.turns.map((x, i) =>
        i === this.turns.length - 1 && x.tool?.status === "running" ? turn : x,
      );
    } catch (e) {
      this.error = friendlyError(e);
    } finally {
      this.busy = false;
    }
  }

  private severityClass(f: AgentFinding): string {
    if (f.severity === "error") return "error";
    if (f.severity === "warn") return "warn";
    return "";
  }


  private async onToolUiAction(turnIndex: number, actionId: string, tool: ToolCallDto) {
    if (actionId === "apply_proposal" && this.turns[turnIndex]?.proposal) {
      await this.onAcceptProposal(turnIndex, this.turns[turnIndex].proposal!);
      return;
    }
    if (actionId === "undo_proposal" && this.turns[turnIndex]?.proposal) {
      await this.onUndoProposal(turnIndex, this.turns[turnIndex].proposal!);
      return;
    }
    if (actionId === "open_download") {
      const payload = tool.payload as { query?: string; tab?: string; theme_zh?: string } | null;
      const q = payload?.query?.trim() || payload?.theme_zh?.trim() || "";
      if (q) this.goDownloadSearch(q, payload?.tab || "mod");
      else this.dispatchEvent(
        new CustomEvent("nav-change", { detail: "download", bubbles: true, composed: true }),
      );
      return;
    }
    if (actionId === "open_folder" || actionId === "reveal_path") {
      if (tool.path && (tool.name === "open_latest_log" || tool.name === "open_log" || tool.name === "diagnostics" || tool.name === "build_pack")) {
        showToast(`路径：${tool.path}`);
        return;
      }
      if (this.instanceId) {
        try {
          await openInstanceFolder(this.instanceId, tool.name === "open_latest_log" ? "game" : "root");
          showToast("已打开文件夹");
        } catch (e) {
          showToast(friendlyError(e));
        }
      } else if (tool.path) {
        showToast(`路径：${tool.path}`);
      }
      return;
    }
    if (actionId === "run_diagnose") {
      await this.onDiagnose();
      return;
    }
    if (actionId === "run_check_mods") {
      await this.onAsk("已装模组", runningTool("check_mods"));
      return;
    }
    if (actionId === "open_home") {
      this.dispatchEvent(
        new CustomEvent("nav-change", { detail: "home", bubbles: true, composed: true }),
      );
      return;
    }
    if (actionId === "open_accounts") {
      this.dispatchEvent(
        new CustomEvent("nav-change", { detail: "accounts", bubbles: true, composed: true }),
      );
    }
  }

  private renderToolCard(turn: ChatTurn, index: number) {
    const tool = turn.tool;
    if (!tool) return nothing;
    const st = tool.status;
    const chipClass =
      st === "ok" ? "ok" : st === "error" ? "error" : st === "running" ? "running" : "pending";
    const isPropose = tool.name === "propose_changes" && turn.proposal;
    const args = [...(tool.args ?? [])];
    if (tool.path && !args.some((a) => a.value === tool.path)) {
      args.push({ key: "path", label_zh: "路径", value: String(tool.path) });
    }
    const resultFallback =
      st === "running"
        ? null
        : st === "error"
          ? "这次没做成，可以换个说法再试，或点快捷工具。"
          : st === "pending"
            ? "需要你确认后才会改动。"
            : "完成。";
    const resultText = (tool.result_zh && tool.result_zh.trim()) || resultFallback;
    return html`
      <div
        class="tool-card ${st === "error" ? "error" : ""} ${st === "running" ? "running" : ""} ${st === "pending" ? "pending" : ""}"
        data-tool-id=${tool.id}
        data-tool-name=${tool.name}
      >
        <div class="tool-card-head">
          <div>
            <h3>${tool.title_zh}</h3>
            ${tool.tag_zh ? html`<p class="proposal-sub">${tool.tag_zh}</p>` : nothing}
          </div>
          <span class="tool-chip ${chipClass}">${statusLabel(st)}</span>
        </div>
        ${st === "running"
          ? html`<div class="tool-progress"><span class="dot" aria-hidden="true"></span>进行中…</div>`
          : nothing}
        ${!isPropose && args.length
          ? html`<dl class="tool-args">
              ${args.map(
                (a) => html`<dt>${a.label_zh}</dt><dd>${a.value}</dd>`,
              )}
            </dl>`
          : nothing}
        ${isPropose
          ? this.renderProposalBody(turn, index)
          : resultText
            ? html`<p class="tool-result ${tool.result_zh ? "" : "muted-fallback"}">${resultText}</p>`
            : nothing}
        ${!isPropose && tool.ui_actions?.length
          ? html`<div class="tool-card-actions">
              ${tool.ui_actions.map(
                (a) => html`
                  <button
                    class=${a.id === "apply_proposal" || a.id === "open_download" ? "primary" : "ghost"}
                    type="button"
                    ?disabled=${this.busy || st === "running" || (a.id === "undo_proposal" && turn.proposalStatus !== "applied") || (a.id === "apply_proposal" && turn.proposalStatus === "applied")}
                    @click=${() => void this.onToolUiAction(index, a.id, tool)}
                  >
                    ${a.label_zh}
                  </button>
                `,
              )}
            </div>`
          : nothing}
      </div>
    `;
  }

  private renderProposalBody(turn: ChatTurn, index: number) {
    const p = turn.proposal;
    if (!p) return nothing;
    const pending = turn.proposalStatus === "pending" || !turn.proposalStatus;
    const applied = turn.proposalStatus === "applied";
    const undone = turn.proposalStatus === "undone";
    return html`
      ${p.subtitle_zh ? html`<p class="proposal-sub">${p.subtitle_zh}</p>` : nothing}
      <table class="proposal-table">
        <thead>
          <tr>
            <th>设置</th>
            <th>当前</th>
            <th>建议</th>
            <th>变化</th>
          </tr>
        </thead>
        <tbody>
          ${(p.items ?? []).map(
            (it) => html`
              <tr>
                <td>${it.label_zh}</td>
                <td>${it.current}</td>
                <td class="proposed">${it.proposed}</td>
                <td class="delta">${it.delta_zh}</td>
              </tr>
            `,
          )}
        </tbody>
      </table>
      <div class="proposal-actions tool-card-actions">
        <button
          class="primary"
          type="button"
          ?disabled=${this.busy || !pending}
          @click=${() => void this.onAcceptProposal(index, p)}
        >
          应用更改
        </button>
        <button
          class="ghost"
          type="button"
          ?disabled=${this.busy || !applied}
          @click=${() => void this.onUndoProposal(index, p)}
        >
          撤销
        </button>
      </div>
      ${applied
        ? html`<p class="proposal-status ok">已应用更改，下次启动生效</p>`
        : undone
          ? html`<p class="proposal-status">已撤销，恢复为应用前的设置</p>`
          : nothing}
    `;
  }

  private renderProposal(turn: ChatTurn, index: number) {
    const p = turn.proposal;
    if (!p) return nothing;
    const pending = turn.proposalStatus === "pending" || !turn.proposalStatus;
    const applied = turn.proposalStatus === "applied";
    const undone = turn.proposalStatus === "undone";
    return html`
      <div class="proposal" data-proposal-id=${p.id}>
        <div class="proposal-head">
          <div>
            <h3>${p.title_zh || "建议更改"}</h3>
            ${p.subtitle_zh
              ? html`<p class="proposal-sub">${p.subtitle_zh}</p>`
              : nothing}
          </div>
          ${p.tag_zh ? html`<span class="tag">${p.tag_zh}</span>` : nothing}
        </div>
        <table class="proposal-table">
          <thead>
            <tr>
              <th>设置</th>
              <th>当前</th>
              <th>建议</th>
              <th>变化</th>
            </tr>
          </thead>
          <tbody>
            ${(p.items ?? []).map(
              (it) => html`
                <tr>
                  <td>${it.label_zh}</td>
                  <td>${it.current}</td>
                  <td class="proposed">${it.proposed}</td>
                  <td class="delta">${it.delta_zh}</td>
                </tr>
              `,
            )}
          </tbody>
        </table>
        <div class="proposal-actions">
          <button
            class="primary"
            type="button"
            ?disabled=${this.busy || !pending}
            @click=${() => void this.onAcceptProposal(index, p)}
          >
            应用更改
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.busy || !applied}
            @click=${() => void this.onUndoProposal(index, p)}
          >
            撤销
          </button>
        </div>
        ${applied
          ? html`<p class="proposal-status ok">已应用更改，下次启动生效</p>`
          : undone
            ? html`<p class="proposal-status">已撤销，恢复为应用前的设置</p>`
            : nothing}
      </div>
    `;
  }

  private goSettingsLlm() {
    sessionStorage.setItem("easymc-settings-section", "other");
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: "settings", bubbles: true, composed: true }),
    );
  }

  private async saveLlm() {
    this.error = "";
    if (!this.llmBase.trim()) {
      this.error = "请填写服务地址，例如 https://api.deepseek.com/v1";
      return;
    }
    if (!this.llmKey.trim() && !this.llmCfg?.api_key_set) {
      this.error = "请粘贴服务商给你的密钥";
      return;
    }
    if (!this.llmModel.trim()) {
      this.error = "请填写模型名称，例如 deepseek-chat";
      return;
    }
    this.llmBusy = true;
    try {
      this.llm = await agentLlmConfigSave(this.llmBase, this.llmKey, this.llmModel);
      this.llmCfg = await agentLlmConfigGet();
      this.llmKey = "";
      if (this.llm.configured) {
        this.showLlmForm = false;
        this.turns = [];
        showToast("助手已连上");
      } else {
        showToast("已保存，但仍未就绪");
      }
    } catch (e) {
      this.error = friendlyError(e);
    } finally {
      this.llmBusy = false;
    }
  }

  private async clearLlm() {
    this.llmBusy = true;
    try {
      this.llm = await agentLlmConfigClear();
      this.llmCfg = await agentLlmConfigGet();
      this.llmBase = "";
      this.llmKey = "";
      this.llmModel = "deepseek-chat";
      this.showLlmForm = true;
      this.turns = [];
      this.report = null;
      showToast("已清除，请重新配置云端助手");
    } catch (e) {
      this.error = friendlyError(e);
    } finally {
      this.llmBusy = false;
    }
  }

  private onSkillHint(hint: { label: string; prompt: string; tool?: string }) {
    if (hint.tool === "search_mods") {
      void this.onSearchModsPrompt();
      return;
    }
    const running = hint.tool ? runningTool(hint.tool) : undefined;
    void this.onAsk(hint.prompt, running);
  }

  private modeMeta(mode?: string) {
    if (!mode || mode === "running") return nothing;
    if (mode.startsWith("llm")) return html`<div class="meta">云端助手</div>`;
    return html`<div class="meta">本地助手</div>`;
  }

  private renderCloudGate() {
    return html`
      <div class="gate" role="dialog" aria-labelledby="llm-gate-title">
        <h2 id="llm-gate-title">先连上助手</h2>
        <p class="muted">
          填好下面三项就能开聊。密钥只保存在本机，不会上传到别处。
          ${this.llmCfg?.api_key_set
            ? html` · 已保存密钥：${this.llmCfg.api_key_hint}`
            : nothing}
        </p>
        ${this.error ? html`<p class="err">${this.error}</p>` : nothing}
        <label class="row">
          <span>服务地址</span>
          <input
            placeholder="例如 https://api.deepseek.com/v1"
            .value=${this.llmBase}
            @input=${(e: Event) => (this.llmBase = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row">
          <span>密钥</span>
          <input
            type="password"
            autocomplete="off"
            placeholder=${this.llmCfg?.api_key_set ? "留空则保持已保存的密钥" : "粘贴服务商给你的密钥"}
            .value=${this.llmKey}
            @input=${(e: Event) => (this.llmKey = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row">
          <span>模型</span>
          <input
            placeholder="例如 deepseek-chat"
            .value=${this.llmModel}
            @input=${(e: Event) => (this.llmModel = (e.target as HTMLInputElement).value)}
          />
        </label>
        <div class="row" style="margin-top:12px">
          <button
            class="primary"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.saveLlm()}
          >
            保存并进入助手
          </button>
          <button class="ghost" type="button" @click=${() => this.goSettingsLlm()}>
            去设置页
          </button>
        </div>
      </div>
    `;
  }

  render() {
    if (this.booting) {
      return html`<p class="boot-muted" role="status">正在检查助手连接…</p>`;
    }
    const llmOn = this.cloudReady;
    if (!llmOn) {
      return html`
        <div class="gate-shell">
          <div class="head">
            <div>
              <h1>
                助手
                <span class="badge">还没连上</span>
              </h1>
              <p class="sub">连上之后就能帮你看崩溃和启动问题。</p>
            </div>
          </div>
          <div class="gate-body">${this.renderCloudGate()}</div>
        </div>
      `;
    }
    const chatEmpty = !this.turns.length && !this.report;
    return html`
      <div class="layout">
        <div class="main-col">
          <div class="head">
            <div>
              <h1>
                助手
                <span class="badge on">云端助手已连接</span>
              </h1>
              <p class="sub">游戏崩了、启动不了？让助手帮你看看哪里坏了。</p>
              <div class="row" style="margin-top:8px">
                <button
                  class="ghost"
                  type="button"
                  @click=${() => (this.showLlmForm = !this.showLlmForm)}
                >
                  ${this.showLlmForm ? "收起连接设置" : "更改连接"}
                </button>
                <button
                  class="ghost"
                  type="button"
                  ?disabled=${this.llmBusy}
                  @click=${() => void this.clearLlm()}
                >
                  清除配置
                </button>
              </div>
            </div>
            <label class="row" style="margin:0">
              <select
                .value=${this.instanceId ?? ""}
                @change=${(e: Event) => {
                  this.instanceId = (e.target as HTMLSelectElement).value || null;
                }}
              >
                <option value="">当前版本</option>
                ${this.instances.map(
                  (i) => html`<option value=${i.id}>${i.name}</option>`,
                )}
              </select>
            </label>
          </div>

          ${this.error ? html`<p class="err">${this.error}</p>` : null}

          ${this.showLlmForm
            ? html`<div class="card" style="margin-bottom:12px">
                <h3 style="margin:0 0 6px">助手连接</h3>
                <p class="muted" style="margin:0 0 10px">
                  改完点保存即可，密钥只留在本机。
                  ${this.llmCfg?.api_key_set
                    ? html`当前密钥：${this.llmCfg.api_key_hint}`
                    : nothing}
                </p>
                <label class="row" style="width:100%; margin-bottom:8px">
                  <span style="min-width:5.5em">服务地址</span>
                  <input
                    style="flex:1"
                    placeholder="例如 https://api.deepseek.com/v1"
                    .value=${this.llmBase}
                    @input=${(e: Event) => (this.llmBase = (e.target as HTMLInputElement).value)}
                  />
                </label>
                <label class="row" style="width:100%; margin-bottom:8px">
                  <span style="min-width:5.5em">密钥</span>
                  <input
                    style="flex:1"
                    type="password"
                    autocomplete="off"
                    placeholder=${this.llmCfg?.api_key_set ? "留空则保持已保存的密钥" : "粘贴服务商给你的密钥"}
                    .value=${this.llmKey}
                    @input=${(e: Event) => (this.llmKey = (e.target as HTMLInputElement).value)}
                  />
                </label>
                <label class="row" style="width:100%; margin-bottom:8px">
                  <span style="min-width:5.5em">模型</span>
                  <input
                    style="flex:1"
                    placeholder="例如 deepseek-chat"
                    .value=${this.llmModel}
                    @input=${(e: Event) => (this.llmModel = (e.target as HTMLInputElement).value)}
                  />
                </label>
                <div class="row">
                  <button
                    class="primary"
                    type="button"
                    ?disabled=${this.llmBusy}
                    @click=${() => void this.saveLlm()}
                  >
                    保存
                  </button>
                </div>
              </div>`
            : nothing}

          ${this.report
            ? html`
                <div class="findings">
                  <p><strong>${this.report.summary_zh}</strong></p>
                  ${(this.report.findings ?? []).map(
                    (f) => html`
                      <div class="card ${this.severityClass(f)}">
                        <h3>${f.title}</h3>
                        <p>${f.summary}</p>
                        <ul>
                          ${(f.fixes ?? []).map((x) => html`<li>${x}</li>`)}
                        </ul>
                        ${f.evidence
                          ? html`<p class="evidence">${f.evidence}</p>`
                          : null}
                      </div>
                    `,
                  )}
                </div>
              `
            : null}

          ${chatEmpty
            ? html`<div class="chat-empty">
                <easymc-empty-state
                  variant="panel"
                  title="有什么我能帮你的？"
                  description="直接提问，或点下方「可以这样说」。窄屏时右侧工具会收起，快捷芯片仍可用。"
                ></easymc-empty-state>
              </div>`
            : html`<div class="chat">
                ${this.turns.map(
                  (t, idx) => html`
                    <div class="msg ${t.role}">
                      <div class="avatar" aria-hidden="true">${t.role === "user" ? "你" : "助"}</div>
                      <div>
                        <div class="bubble">${t.content}</div>
                        ${t.role === "assistant"
                          ? (t.tools?.length
                              ? html`<div class="tool-stack">${t.tools.map((tc) => this.renderToolCard({ ...t, tool: tc }, idx))}</div>`
                              : t.tool
                                ? this.renderToolCard(t, idx)
                                : this.renderProposal(t, idx))
                          : nothing}
                        ${this.modeMeta(t.mode)}
                      </div>
                    </div>
                  `,
                )}
              </div>`}

          ${this.pendingStoreQuery
            ? html`<div class="quick-chips" role="group" aria-label="安装失败后的操作">
                <button
                  class="chip primary"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.goDownloadSearch(this.pendingStoreQuery || "")}
                >
                  去下载页搜索「${this.pendingStoreQuery}」
                </button>
              </div>`
            : null}

          ${chatEmpty
            ? html`<div class="skills-hints" role="group" aria-label="可以这样说">
                <p class="hint-label">可以这样说</p>
                <div class="hint-row">
                  ${SKILL_HINTS.map(
                    (h) => html`
                      <button
                        class="chip"
                        type="button"
                        ?disabled=${this.busy}
                        @click=${() => this.onSkillHint(h)}
                      >
                        ${h.label}
                      </button>
                    `,
                  )}
                </div>
              </div>`
            : nothing}

          <div class="quick-chips" role="group" aria-label="快捷工具">
            <button class="chip primary" type="button" ?disabled=${this.busy} @click=${() => this.goDownloadCenter()}>去下载中心</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onSetupEnv()}>一键环境</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>一键查崩溃</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onSearchModsPrompt()}>搜模组</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onBuildPack()}>导出整合包</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onTuneConfig()}>优化配置</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onBumpMem()}>内存 4 GB</button>
          </div>

          <div class="composer">
            <input
              placeholder="说说遇到什么问题…"
              .value=${this.input}
              ?disabled=${this.busy}
              @input=${(e: Event) => {
                this.input = (e.target as HTMLInputElement).value;
              }}
              @keydown=${(e: KeyboardEvent) => {
                if (e.key === "Enter") void this.onAsk();
              }}
            />
            <button class="primary" ?disabled=${this.busy} @click=${() => this.onAsk()}>发送</button>
          </div>
        </div>

        <aside class="tools" aria-label="快捷工具">
          <h2>快捷工具</h2>
          <div class="tool-stack">
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onSetupEnv()}>
              <span class="tool-copy"><strong>一键环境</strong><span>准备 Java，汇报内存与下载源</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>
              <span class="tool-copy"><strong>一键查崩溃</strong><span>分析崩溃 / 启动日志，给出修法</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onSearchModsPrompt()}>
              <span class="tool-copy"><strong>搜模组</strong><span>在 Modrinth 搜索，结果可再说「装 xxx」</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onSuggestMods()}>
              <span class="tool-copy"><strong>推荐模组</strong><span>按生存 / 科技 / 魔法 / 性能主题推荐</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onTuneConfig()}>
              <span class="tool-copy"><strong>优化配置</strong><span>建议内存与 JVM，确认后应用</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onBuildPack()}>
              <span class="tool-copy"><strong>导出整合包</strong><span>制作当前版本 Modrinth .mrpack</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button
              class="tool"
              ?disabled=${this.busy}
              @click=${() => this.onAsk("版本列表", runningTool("list_instances"))}
            >
              <span class="tool-copy"><strong>版本列表</strong><span>看看已安装了哪些版本</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button
              class="tool"
              ?disabled=${this.busy}
              @click=${() => this.onAsk("打开日志", runningTool("open_latest_log"))}
            >
              <span class="tool-copy"><strong>打开日志</strong><span>定位 latest.log / 崩溃报告</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button
              class="tool"
              ?disabled=${this.busy}
              @click=${() => this.onAsk("账户列表", runningTool("list_accounts"))}
            >
              <span class="tool-copy"><strong>账户列表</strong><span>看看已保存的正版 / 离线账号</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onBmclapi()}>
              <span class="tool-copy"><strong>切换 BMCLAPI 加速</strong><span>国内镜像下载游戏文件</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onOpenInstanceDir()}>
              <span class="tool-copy"><strong>打开版本文件夹</strong><span>去改配置或找存档</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onCheckMods()}>
              <span class="tool-copy"><strong>已装模组</strong><span>列出当前版本 mods 文件夹</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
          </div>
          <p class="note">「优化配置」只给出建议，点「应用更改」后才会改动；「撤销」可恢复。都在你这台电脑上完成。</p>
        </aside>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-agent-page": AgentPage;
  }
}
