import { LitElement, css, html } from "lit";
import { controlStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentAsk,
  agentBumpMemory,
  agentDataDir,
  agentDiagnose,
  agentExportDiagnostics,
  agentLlmStatus,
  ensureJava,
  listInstancesState,
} from "../api";
import { showToast } from "../toast";
import type {
  AgentAskResponse,
  AgentFinding,
  DiagnoseReportDto,
  InstanceDto,
  LlmStatusDto,
} from "../types";

type ChatTurn = { role: "user" | "assistant"; content: string; mode?: string };

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

  static styles = [
    controlStyles,
    css`
    :host {
      display: block;
      max-width: 960px;
      height: calc(100vh - 100px);
      min-height: 480px;
    }
    .layout {
      display: grid;
      grid-template-columns: 1fr 220px;
      gap: 16px;
      height: 100%;
      min-height: 0;
    }
    @media (max-width: 820px) {
      .layout { grid-template-columns: 1fr; }
      .tools { display: none; }
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
      gap: 12px;
      margin-bottom: 12px;
    }
    h1 {
      margin: 0;
      font-size: 1.35rem;
      font-weight: 700;
      letter-spacing: -0.03em;
      display: flex;
      align-items: center;
      gap: 10px;
      flex-wrap: wrap;
    }
    .sub {
      margin: 4px 0 0;
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.9rem;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.75rem;
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
      padding: 8px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      font: inherit;
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    /* primary → controlStyles (CMP-1); keep border reset for agent base button */
    button.primary { border-color: transparent; }
    button:disabled { opacity: 0.5; cursor: not-allowed; }
    button:active:not(:disabled), .chip:active { transform: scale(0.98); }
    select {
      padding: 8px 12px;
      border-radius: 12px;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
    }
    .chat {
      flex: 1;
      display: flex;
      flex-direction: column;
      gap: 12px;
      overflow: auto;
      padding: 16px;
      border-radius: 18px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elevation, no border */
      margin-bottom: 12px;
      min-height: 240px;
    }
    .msg {
      display: flex;
      gap: 10px;
      align-items: flex-start;
      max-width: 92%;
    }
    .msg.user { align-self: flex-end; flex-direction: row-reverse; }
    .msg.assistant { align-self: flex-start; }
    .avatar {
      width: 32px;
      height: 32px;
      border-radius: 50%;
      flex-shrink: 0;
      display: grid;
      place-items: center;
      font-size: 0.75rem;
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
      font-size: 0.92rem;
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
      font-size: 0.7rem;
      opacity: 0.7;
      margin-top: 4px;
    }
    .composer {
      display: flex;
      gap: 8px;
      align-items: center;
    }
    .composer input {
      flex: 1;
      padding: 13px 16px;
      border-radius: 999px;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
      /* CF-1: border only for input */
    }
    .tools {
      display: flex;
      flex-direction: column;
      gap: 8px;
      padding: 14px;
      border-radius: 18px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elevation, no border */
      align-self: start;
    }
    .tools h2 {
      margin: 0 0 4px;
      font-size: 0.85rem;
      font-weight: 650;
      color: var(--md-sys-color-on-surface-variant);
      letter-spacing: 0.02em;
      text-transform: none;
    }
    button.tool {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      gap: 2px;
      text-align: left;
      width: 100%;
      padding: 12px;
      border-radius: 14px;
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      background: var(--md-sys-color-surface);
    }
    button.tool strong {
      font-size: 0.88rem;
      font-weight: 650;
      color: var(--md-sys-color-on-surface);
    }
    button.tool span {
      font-size: 0.75rem;
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.3;
    }
    button.tool:hover {
      background: var(--md-sys-color-primary-container);
      border-color: transparent;
    }
    .tools .note {
      font-size: 0.72rem;
      color: var(--md-sys-color-on-surface-variant);
      margin-top: 4px;
      line-height: 1.35;
    }
    .findings {
      display: flex;
      flex-direction: column;
      gap: 10px;
      margin: 0 0 12px;
    }
    .card {
      padding: 14px 16px;
      border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
    }
    .card.error { outline: 1px solid var(--md-sys-color-error); }
    .card.warn { outline: 1px solid var(--md-sys-color-warning); }
    .card h3 { margin: 0 0 6px; font-size: 1rem; }
    .card p { margin: 0 0 6px; font-size: 0.9rem; color: var(--md-sys-color-on-surface-variant); }
    .card ul { margin: 0; padding-left: 1.2em; font-size: 0.88rem; }
    .evidence {
      font-family: ui-monospace, monospace;
      font-size: 0.75rem;
      opacity: 0.85;
      word-break: break-all;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
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
    const [st, inst] = await Promise.all([agentLlmStatus(), listInstancesState()]);
    this.llm = st;
    this.instances = inst.instances;
    this.instanceId = inst.active_id;
    this.turns = [
      {
        role: "assistant",
        content: st.configured
          ? "本地助手和云端助手都已就绪。可以直接提问，或点「帮我看看上次为什么崩了」。"
          : "本地助手已就绪，不联网也能用。可以说「看上次报错」「确保 Java 21」「装 sodium」「内存 4G」。",
        mode: st.mode,
      },
    ];
  }

  private async onDiagnose() {
    this.busy = true;
    this.error = "";
    try {
      const report = await agentDiagnose(this.instanceId);
      this.report = report;
      this.turns = [
        ...this.turns,
        { role: "user", content: "帮我看看上次为什么崩了" },
        { role: "assistant", content: this.formatReport(report), mode: "rules" },
      ];
      showToast(report.summary_zh.slice(0, 80));
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  private formatReport(r: DiagnoseReportDto): string {
    let s = r.summary_zh;
    for (const f of r.findings ?? []) {
      s += `\n\n【${f.title}】${f.summary}\n建议：\n- ${(f.fixes ?? []).join("\n- ")}`;
      if (f.evidence) s += `\n证据：${f.evidence}`;
    }
    return s;
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
      this.turns = [
        ...this.turns,
        { role: "assistant", content: resp.reply_zh, mode: resp.mode },
      ];
      if (resp.action?.action === "ensure_java") {
        showToast("正在检查 Java…");
      }
    } catch (e) {
      this.error = String(e);
      this.turns = [...this.turns, { role: "assistant", content: `错误：${e}` }];
    } finally {
      this.busy = false;
    }
  }

  private async onEnsureJava() {
    this.busy = true;
    try {
      const msg = await ensureJava(21);
      showToast(msg);
      this.turns = [...this.turns, { role: "assistant", content: `Java 已就绪：${msg}` }];
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  private async onDataDir() {
    const r = await agentDataDir();
    showToast(r.message);
    this.turns = [...this.turns, { role: "assistant", content: r.message }];
  }

  private async onBumpMem() {
    this.busy = true;
    try {
      const r = await agentBumpMemory(4096);
      showToast(r.message);
      this.turns = [...this.turns, { role: "assistant", content: r.message }];
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  private async onExport() {
    this.busy = true;
    try {
      const r = await agentExportDiagnostics(this.instanceId);
      showToast(r.message);
      this.turns = [...this.turns, { role: "assistant", content: r.message }];
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

          <div class="chat">
            ${this.turns.map(
              (t) => html`
                <div class="msg ${t.role}">
                  <div class="avatar" aria-hidden="true">${t.role === "user" ? "你" : "助"}</div>
                  <div>
                    <div class="bubble">${t.content}</div>
                    ${t.mode
                      ? html`<div class="meta">${t.mode.startsWith("llm") ? "云端助手" : "本地助手"}</div>`
                      : null}
                  </div>
                </div>
              `,
            )}
          </div>

          <div class="composer">
            <input
              placeholder=${llmOn
                ? "说说遇到什么问题…"
                : "例如：看上次报错 / 修好 Java / 内存 4G"}
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
          <button class="tool" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>
            <strong>看上次报错</strong>
            <span>分析崩溃日志，给出修法</span>
          </button>
          <button class="tool" ?disabled=${this.busy} @click=${() => this.onEnsureJava()}>
            <strong>修好 Java</strong>
            <span>检查并准备合适的 Java</span>
          </button>
          <button class="tool" ?disabled=${this.busy} @click=${() => this.onBumpMem()}>
            <strong>内存调到 4 GB</strong>
            <span>给游戏多留一点内存</span>
          </button>
          <button class="tool" ?disabled=${this.busy} @click=${() => this.onDataDir()}>
            <strong>打开游戏文件夹</strong>
            <span>去改配置或找存档</span>
          </button>
          <button class="tool" ?disabled=${this.busy} @click=${() => this.onExport()}>
            <strong>导出问题报告</strong>
            <span>发给朋友或自己留档</span>
          </button>
          <p class="note">工具在你这台电脑上执行，不会乱改游戏以外的东西。</p>
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
