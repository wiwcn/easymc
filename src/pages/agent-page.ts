import { LitElement, css, html } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
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
import "../components/empty-state";
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
    pageChromeStyles,
    css`
    :host {
      display: block;
      max-width: 960px;
      height: calc(100vh - 100px);
      min-height: 480px;
    }
    .layout {
      display: grid;
      grid-template-columns: 1fr 240px;
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
      min-height: 32px; /* A11Y-4 */
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
    /* primary → controlStyles (CMP-1); keep border reset for agent base button */
    button.primary { border-color: transparent; }
    button:disabled { opacity: 0.5; cursor: not-allowed; }
    button:active:not(:disabled), .chip:active { transform: scale(0.98); }
    select {
      padding: 8px 12px;
      border-radius: var(--md-sys-shape-corner-small); /* CMP-5: form field */
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
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elevation, no border */
      margin-bottom: var(--easymc-space-3);
      min-height: 240px;
    }
    .msg {
      display: flex;
      gap: var(--easymc-space-3);
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
      color: var(--md-sys-color-on-surface-variant); /* A11Y-3: no opacity stack */
      margin-top: 4px;
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
      /* CF-1: border only for input */
    }
    .tools {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container);
      box-shadow: none; /* AG-1: tray; cards carry elevation */
      border: none;
      align-self: start;
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
      box-shadow: var(--md-sys-elevation-1); /* AG-1 card stack */
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
    /* ADAPT-1: chips when tools column hidden */
    .quick-chips {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
      margin-bottom: var(--easymc-space-3);
    }
    /* ADAPT-1: after .tools {display:flex} so hide wins */
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
    const [st, inst] = await Promise.all([agentLlmStatus(), listInstancesState()]);
    this.llm = st;
    this.instances = inst.instances;
    this.instanceId = inst.active_id;
    this.turns = [
      {
        role: "assistant",
        content: st.configured
          ? "本地 + 云端助手已就绪。点快捷操作，或直接提问。"
          : "本地助手已就绪。点快捷操作，或直接提问。",
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

          ${!this.turns.length && !this.report
            ? html`<div class="chat-empty">
                <easymc-empty-state
                  variant="panel"
                  title="有什么我能帮你的？"
                  description="说说启动不了、崩了，或点下方快捷操作"
                ></easymc-empty-state>
              </div>`
            : html`<div class="chat">
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
              </div>`}

          <div class="quick-chips" role="group" aria-label="快捷工具">
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>看上次报错</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onEnsureJava()}>修好 Java</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onBumpMem()}>内存 4 GB</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onDataDir()}>游戏文件夹</button>
            <button class="chip" type="button" ?disabled=${this.busy} @click=${() => this.onExport()}>导出报告</button>
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
          <div class="tool-stack">
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>
              <span class="tool-copy"><strong>看上次报错</strong><span>分析崩溃日志，给出修法</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onEnsureJava()}>
              <span class="tool-copy"><strong>修好 Java</strong><span>检查并准备合适的 Java</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onBumpMem()}>
              <span class="tool-copy"><strong>内存调到 4 GB</strong><span>给游戏多留一点内存</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onDataDir()}>
              <span class="tool-copy"><strong>打开游戏文件夹</strong><span>去改配置或找存档</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
            <button class="tool" ?disabled=${this.busy} @click=${() => this.onExport()}>
              <span class="tool-copy"><strong>导出问题报告</strong><span>发给朋友或自己留档</span></span>
              <span class="chev" aria-hidden="true">›</span>
            </button>
          </div>
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
