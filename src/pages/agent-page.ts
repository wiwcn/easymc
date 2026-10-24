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
} from "../types";

type ChatTurn = {
  role: "user" | "assistant";
  content: string;
  mode?: string;
  proposal?: ChangeProposalDto | null;
  /** applied | undone | pending */
  proposalStatus?: "pending" | "applied" | "undone";
};

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

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    :host {
      display: block;
      max-width: 1080px;
      height: calc(100vh - 100px);
      min-height: 480px;
    }
    .layout {
      display: grid;
      grid-template-columns: 1fr 260px;
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
      font-weight: 600;
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
    select {
      padding: 8px 12px;
      border-radius: var(--md-sys-shape-corner-small);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
    }
    .chat-empty {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 240px;
      margin-bottom: var(--easymc-space-3);
    }
    .chat {
      flex: 1;
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-3);
      overflow: auto;
      padding: var(--easymc-space-4);
      border-radius: 18px;
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
      font-weight: 700;
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
      border-radius: 16px;
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
    /* Proposal card (ghost card toward agent-v1) */
    .proposal {
      margin-top: var(--easymc-space-3);
      padding: var(--easymc-space-4);
      border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      border: 1px solid color-mix(in srgb, var(--md-sys-color-outline-variant) 55%, transparent);
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
      font-weight: 650;
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
      font-weight: 600;
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
      font-weight: 600;
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta);
    }
    .proposal-table .delta {
      color: var(--md-sys-color-primary);
      font-weight: 600;
    }
    .proposal-table .proposed {
      font-weight: 650;
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
      font-weight: 600;
    }
    .composer {
      display: flex;
      gap: var(--easymc-space-2);
      align-items: center;
    }
    .composer input {
      flex: 1;
      padding: 13px 16px;
      border-radius: 999px;
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
      background: var(--md-sys-color-surface-container);
      box-shadow: none;
      border: none;
      align-self: start;
      max-height: 100%;
      overflow: auto;
    }
    .tools h2 {
      margin: 0 0 var(--easymc-space-2);
      font-size: var(--easymc-type-label);
      font-weight: 650;
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
      font-weight: 650;
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
    @media (max-width: 820px) {
      .layout { grid-template-columns: 1fr; }
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
      border-radius: 16px;
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
  `,
  ];

  connectedCallback() {
    super.connectedCallback();
    void this.boot();
  }

  private async boot() {
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
    this.showLlmForm = !st.configured;
    this.instances = inst.instances;
    this.instanceId = inst.active_id;
    this.turns = [
      {
        role: "assistant",
        content: st.configured
          ? "本地 + 云端助手已就绪。点右侧工具，或直接提问。"
          : "本地助手已就绪。点右侧工具，或直接提问。",
        mode: st.mode,
      },
    ];
    if (sessionStorage.getItem("easymc-agent-autodiagnose") === "1") {
      sessionStorage.removeItem("easymc-agent-autodiagnose");
      void this.onDiagnose();
    }
  }

  private async onDiagnose() {
    this.busy = true;
    this.error = "";
    try {
      // Prefer ask path so OOM can attach propose_changes.
      await this.onAsk("查看崩溃");
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }


  private async onAsk(text?: string) {
    const q = (text ?? this.input).trim();
    if (!q || this.busy) return;
    this.input = "";
    this.busy = true;
    this.error = "";
    this.turns = [...this.turns, { role: "user", content: q }];
    try {
      const history = this.turns
        .filter((t) => t.role === "user" || t.role === "assistant")
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
      if (resp.action) {
        if (resp.action.action === "propose_changes") {
          proposal = asProposal(resp.action.value);
          proposalStatus = proposal ? "pending" : undefined;
          // Don't run silent side effects for proposals.
        } else {
          content = this.enrichActionReply(content, resp.action);
          void this.applyAction(resp.action);
        }
      }
      this.turns = [
        ...this.turns,
        {
          role: "assistant",
          content,
          mode: resp.mode,
          proposal,
          proposalStatus,
        },
      ];
    } catch (e) {
      this.error = String(e);
      this.turns = [...this.turns, { role: "assistant", content: `错误：${e}` }];
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
    if (action.action === "build_pack") {
      showToast(action.ok ? "整合包已生成" : "生成整合包失败");
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
      return {
        ...t,
        proposalStatus: status,
        content: extraContent ? `${t.content}\n\n${extraContent}` : t.content,
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
      this.error = String(e);
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
      this.error = String(e);
      showToast("撤销失败");
    } finally {
      this.busy = false;
    }
  }

  private async onEnsureJava() {
    await this.onAsk("修好 Java");
  }

  private async onSetupEnv() {
    await this.onAsk("一键环境");
  }

  private async onTuneConfig() {
    await this.onAsk("优化配置");
  }

  private async onBuildPack() {
    await this.onAsk("生成整合包");
  }

  private async onBmclapi() {
    await this.onAsk("切换 BMCLAPI 加速");
  }

  private async onOpenInstanceDir() {
    await this.onAsk("打开版本文件夹");
  }

  private async onCheckMods() {
    await this.onAsk("检查模组");
  }

  private async onDataDir() {
    this.turns = [...this.turns, { role: "user", content: "游戏文件夹在哪" }];
    const r = await agentDataDir();
    showToast(r.message.slice(0, 60));
    this.turns = [...this.turns, { role: "assistant", content: r.message, mode: "rules" }];
  }

  private async onBumpMem() {
    this.busy = true;
    try {
      this.turns = [...this.turns, { role: "user", content: "内存调到 4 GB" }];
      const r = await agentBumpMemory(4096);
      const content = r.ok
        ? `${r.message}\n\n下一步：回到启动页，重新点「启动游戏」。`
        : r.message;
      showToast(r.ok ? "内存已调整" : "调整失败");
      this.turns = [...this.turns, { role: "assistant", content, mode: "rules" }];
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  private async onExport() {
    this.busy = true;
    try {
      this.turns = [...this.turns, { role: "user", content: "导出问题报告" }];
      const r = await agentExportDiagnostics(this.instanceId);
      showToast(r.ok ? "报告已导出" : "导出失败");
      this.turns = [...this.turns, { role: "assistant", content: r.message, mode: "rules" }];
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  private severityClass(f: AgentFinding): string {
    if (f.severity === "error") return "error";
    if (f.severity === "warn") return "warn";
    return "";
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
            <h3>✨ ${p.title_zh || "建议更改"}</h3>
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
            ✓ 应用更改
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.busy || (!applied && !pending)}
            @click=${() => void this.onUndoProposal(index, p)}
          >
            ↺ 撤销
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
    this.llmBusy = true;
    this.error = "";
    try {
      this.llm = await agentLlmConfigSave(this.llmBase, this.llmKey, this.llmModel);
      this.llmCfg = await agentLlmConfigGet();
      this.llmKey = "";
      showToast(this.llm.configured ? "云端助手已启用" : "已保存");
      if (this.llm.configured) this.showLlmForm = false;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
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
      showToast("已清除云端助手配置");
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.llmBusy = false;
    }
  }

  render() {
    const llmOn = this.llm?.configured;
    return html`
      <div class="layout">
        <div class="main-col">
          <div class="head">
            <div>
              <h1>
                助手
                <span class="badge ${llmOn ? "on" : ""}">
                  ${llmOn ? "云端助手已连接" : "本地助手已就绪"}
                </span>
              </h1>
              <p class="sub">游戏崩了、启动不了？让助手帮你看看哪里坏了。</p>
              <div class="row" style="margin-top:8px">
                <button
                  class="ghost"
                  type="button"
                  @click=${() => (this.showLlmForm = !this.showLlmForm)}
                >
                  ${this.showLlmForm ? "收起 API 配置" : "配置云端 API"}
                </button>
                <button class="ghost" type="button" @click=${() => this.goSettingsLlm()}>
                  设置里配置
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
                <h3 style="margin:0 0 6px">云端助手 API</h3>
                <p class="muted" style="margin:0 0 10px">
                  OpenAI 兼容接口。保存到本机 <code>~/.config/easymc/llm.env</code>。
                  ${this.llmCfg?.api_key_set
                    ? html`当前密钥：${this.llmCfg.api_key_hint}`
                    : "尚未保存密钥。"}
                </p>
                <label class="row" style="width:100%; margin-bottom:8px">
                  <span style="min-width:5.5em">Base URL</span>
                  <input
                    style="flex:1"
                    placeholder="https://api.example.com"
                    .value=${this.llmBase}
                    @input=${(e: Event) => (this.llmBase = (e.target as HTMLInputElement).value)}
                  />
                </label>
                <label class="row" style="width:100%; margin-bottom:8px">
                  <span style="min-width:5.5em">API Key</span>
                  <input
                    style="flex:1"
                    type="password"
                    autocomplete="off"
                    placeholder=${this.llmCfg?.api_key_set ? "留空则保持已保存密钥" : "sk-…"}
                    .value=${this.llmKey}
                    @input=${(e: Event) => (this.llmKey = (e.target as HTMLInputElement).value)}
                  />
                </label>
                <label class="row" style="width:100%; margin-bottom:8px">
                  <span style="min-width:5.5em">Model</span>
                  <input
                    style="flex:1"
                    placeholder="deepseek-chat"
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
                    保存并启用
                  </button>
                  <button
                    class="ghost"
                    type="button"
                    ?disabled=${this.llmBusy}
                    @click=${() => void this.clearLlm()}
                  >
                    清除
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

          ${!this.turns.length && !this.report
            ? html`<div class="chat-empty">
                <easymc-empty-state
                  variant="panel"
                  title="有什么我能帮你的？"
                  description="说说启动不了、崩了，或点右侧 / 下方快捷工具"
                ></easymc-empty-state>
              </div>`
            : html`<div class="chat">
                ${this.turns.map(
                  (t, idx) => html`
                    <div class="msg ${t.role}">
                      <div class="avatar" aria-hidden="true">${t.role === "user" ? "你" : "助"}</div>
                      <div>
                        <div class="bubble">${t.content}</div>
                        ${t.role === "assistant" ? this.renderProposal(t, idx) : nothing}
                        ${t.mode
                          ? html`<div class="meta">${t.mode.startsWith("llm") ? "云端助手" : "本地助手"}</div>`
                          : null}
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

          <div class="quick-chips" role="group" aria-label="快捷工具">
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onSetupEnv()}>一键环境</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>查看崩溃</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onTuneConfig()}>优化配置</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onBuildPack()}>生成整合包</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onBumpMem()}>内存 4 GB</button>
          </div>

          <div class="composer">
            <input
              placeholder=${llmOn
                ? "说说遇到什么问题…"
                : "例如：查看崩溃 / 优化配置 / 装 sodium"}
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
              <span class="tool-copy"><strong>查看崩溃</strong><span>分析崩溃日志，给出修法</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onTuneConfig()}>
              <span class="tool-copy"><strong>优化配置</strong><span>建议内存与 JVM，确认后应用</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onBuildPack()}>
              <span class="tool-copy"><strong>生成整合包</strong><span>导出当前版本分享包</span></span>
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
              <span class="tool-copy"><strong>检查模组</strong><span>统计模组数量，提示是否需诊断</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
          </div>
          <p class="note">「优化配置」只生成建议，需点「应用更改」才会写入。工具在你这台电脑上执行。</p>
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
