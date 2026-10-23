import { LitElement, css, html } from "lit";
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

  static styles = css`
    :host {
      display: block;
      max-width: 920px;
    }
    h1 {
      margin: 0 0 4px;
      font-size: 1.5rem;
    }
    .sub {
      margin: 0 0 16px;
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.95rem;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.8rem;
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
      margin-left: 8px;
    }
    .badge.on {
      background: #c8e6c9;
      color: #1b5e20;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-bottom: 12px;
      align-items: center;
    }
    button,
    .chip {
      padding: 8px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      font: inherit;
    }
    button.primary {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border-color: transparent;
    }
    button:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
    button,
    .chip {
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        opacity var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button:active:not(:disabled),
    .chip:active {
      transform: scale(0.98);
    }
    .badge.on {
      background: var(--md-sys-color-success-container);
      color: var(--md-sys-color-on-success-container);
    }
    .err {
      color: var(--md-sys-color-error);
    }
    .findings .card {
      animation: row-in var(--easymc-dur-med) var(--easymc-ease-out) both;
    }
    @keyframes row-in {
      from { opacity: 0; transform: translateY(6px); }
      to { opacity: 1; transform: none; }
    }
    @media (prefers-reduced-motion: reduce) {
      .findings .card { animation: none; }
      button:active:not(:disabled),
      .chip:active { transform: none; }
    }
    select {
      padding: 8px 12px;
      border-radius: 12px;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
    }
    .findings {
      display: flex;
      flex-direction: column;
      gap: 10px;
      margin: 12px 0 20px;
    }
    .card {
      padding: 14px 16px;
      border-radius: 16px;
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid var(--md-sys-color-outline-variant);
    }
    .card.error {
      border-color: #e57373;
    }
    .card.warn {
      border-color: #ffb74d;
    }
    .card h3 {
      margin: 0 0 6px;
      font-size: 1rem;
    }
    .card p {
      margin: 0 0 6px;
      font-size: 0.9rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .card ul {
      margin: 0;
      padding-left: 1.2em;
      font-size: 0.88rem;
    }
    .evidence {
      font-family: ui-monospace, monospace;
      font-size: 0.75rem;
      opacity: 0.85;
      word-break: break-all;
    }
    .chat {
      display: flex;
      flex-direction: column;
      gap: 10px;
      max-height: 420px;
      overflow: auto;
      padding: 12px;
      border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      border: 1px solid var(--md-sys-color-outline-variant);
      margin-bottom: 12px;
    }
    .bubble {
      max-width: 85%;
      padding: 10px 14px;
      border-radius: 16px;
      white-space: pre-wrap;
      font-size: 0.92rem;
      line-height: 1.45;
    }
    .bubble.user {
      align-self: flex-end;
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .bubble.assistant {
      align-self: flex-start;
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
    }
    .meta {
      font-size: 0.7rem;
      opacity: 0.7;
      margin-top: 4px;
    }
    .composer {
      display: flex;
      gap: 8px;
    }
    .composer input {
      flex: 1;
      padding: 12px 14px;
      border-radius: 24px;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      font: inherit;
    }
    .hints {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      margin-top: 8px;
    }
    .hints button {
      font-size: 0.8rem;
      padding: 6px 10px;
    }
    .err {
      color: #c62828;
      margin: 8px 0;
    }
  `;

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
      <h1>
        助手
        <span class="badge ${llmOn ? "on" : ""}">
          ${llmOn ? "云端助手已连接" : "本地助手已就绪"}
        </span>
      </h1>
      <p class="sub">
        游戏崩了、启动不了？让助手帮你看看哪里坏了。需要更聪明的问答时，可在设置里连接 AI 助手。
      </p>

      <div class="row">
        <label>
          实例
          <select
            .value=${this.instanceId ?? ""}
            @change=${(e: Event) => {
              this.instanceId = (e.target as HTMLSelectElement).value || null;
            }}
          >
            <option value="">当前实例</option>
            ${this.instances.map(
              (i) => html`<option value=${i.id}>${i.name} · ${i.version}</option>`,
            )}
          </select>
        </label>
        <button class="primary" ?disabled=${this.busy} @click=${() => this.onDiagnose()}>
          帮我看看上次为什么崩了
        </button>
        <button ?disabled=${this.busy} @click=${() => this.onEnsureJava()}>修好 Java</button>
        <button ?disabled=${this.busy} @click=${() => this.onBumpMem()}>游戏内存调到 4 GB</button>
        <button ?disabled=${this.busy} @click=${() => this.onDataDir()}>打开游戏文件夹</button>
        <button ?disabled=${this.busy} @click=${() => this.onExport()}>导出问题报告</button>
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
            <div class="bubble ${t.role}">
              ${t.content}
              ${t.mode ? html`<div class="meta">${t.mode.startsWith("llm") ? "云端助手" : "本地助手"}</div>` : null}
            </div>
          `,
        )}
      </div>

      <div class="composer">
        <input
          placeholder=${llmOn
            ? "自由提问，或输入「看上次报错」…"
            : "例如：看上次报错 / 确保 Java 21 / 装 sodium"}
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
      <div class="hints">
        ${["看上次报错", "确保 Java 21", "内存 4G", "装 sodium", "数据目录", "导出诊断包"].map(
          (h) =>
            html`<button ?disabled=${this.busy} @click=${() => this.onAsk(h)}>${h}</button>`,
        )}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-agent-page": AgentPage;
  }
}
