import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import type { InstanceDto } from "../types";
import {
  listInstances,
  launchInstance,
  stopInstance,
  installVersion,
  ensureJava,
  listenLogs,
  listenInstallProgress,
} from "../api";
import { showToast } from "../toast";

@customElement("easymc-home-page")
export class HomePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private loading = true;
  @state() private busy = false;
  @state() private status = "";
  @state() private logs: string[] = [];
  @state() private showLogs = false;

  private unLog?: () => void;
  private unProg?: () => void;

  static styles = css`
    :host {
      display: block;
    }
    .hero {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 24px;
      align-items: stretch;
      padding: 24px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: linear-gradient(
        135deg,
        var(--md-sys-color-primary-container) 0%,
        #e0f2f1 55%,
        var(--md-sys-color-surface-container) 100%
      );
      box-shadow: var(--md-sys-elevation-1);
      margin-bottom: 16px;
    }
    .hero-body {
      display: flex;
      flex-direction: column;
      gap: 10px;
      min-width: 0;
    }
    .thumb {
      width: 96px;
      height: 96px;
      border-radius: var(--md-sys-shape-corner-large);
      background: linear-gradient(160deg, #4db6ac, #00695c);
      display: grid;
      place-items: center;
      font-size: 2.5rem;
      color: white;
      margin-bottom: 8px;
    }
    h1 {
      margin: 0;
      font-size: 1.75rem;
      font-weight: 700;
    }
    .tags {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    .tag {
      padding: 4px 12px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.75rem;
      font-weight: 500;
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface-variant);
    }
    .tag.accent {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .tag.warn {
      background: #fff3e0;
      color: #e65100;
    }
    .desc {
      color: var(--md-sys-color-on-surface-variant);
      margin: 0;
    }
    .meta {
      margin-top: auto;
      padding-top: 12px;
      font-size: 0.85rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .actions {
      display: flex;
      flex-direction: column;
      gap: 10px;
      align-self: center;
    }
    .launch,
    .secondary {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      padding: 12px 24px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.95rem;
      font-weight: 600;
      cursor: pointer;
      white-space: nowrap;
    }
    .launch {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      box-shadow: var(--md-sys-elevation-2);
    }
    .launch:disabled,
    .secondary:disabled {
      opacity: 0.55;
      cursor: not-allowed;
    }
    .secondary {
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface);
    }
    .status {
      margin: 0 0 16px;
      font-size: 0.9rem;
      color: var(--md-sys-color-on-surface-variant);
      min-height: 1.2em;
    }
    .section-head {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      margin-bottom: 14px;
    }
    .section-head h2 {
      margin: 0;
      font-size: 1.15rem;
      font-weight: 600;
    }
    .link {
      color: var(--md-sys-color-primary);
      font-size: 0.9rem;
      cursor: pointer;
      background: none;
      border: none;
      font-weight: 500;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
      gap: 16px;
    }
    .card {
      position: relative;
      padding: 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      min-height: 120px;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .card:nth-child(1) {
      background: #e0f2f1;
    }
    .card:nth-child(2) {
      background: #f5f0e6;
    }
    .card:nth-child(3) {
      background: #e3f2fd;
    }
    .card h3 {
      margin: 0;
      font-size: 1rem;
    }
    .card .ver {
      font-size: 0.8rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .status-line {
      margin-top: auto;
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 0.8rem;
      font-weight: 500;
    }
    .status-line.ready {
      color: var(--md-sys-color-success);
    }
    .status-line.need {
      color: #e65100;
    }
    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: currentColor;
    }
    .logs {
      margin-top: 20px;
      border-radius: var(--md-sys-shape-corner-large);
      background: #111827;
      color: #e5e7eb;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 12px;
      max-height: 280px;
      overflow: auto;
      padding: 12px 14px;
      white-space: pre-wrap;
      word-break: break-all;
    }
    .logs-empty {
      opacity: 0.6;
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.load();
    void listenLogs((line) => {
      this.logs = [...this.logs.slice(-400), line];
      this.showLogs = true;
    }).then((u) => (this.unLog = u));
    void listenInstallProgress((p) => {
      const tot = p.total != null ? `/${p.total}` : "";
      this.status = `[${p.stage}] ${p.current}${tot} ${p.detail}`;
    }).then((u) => (this.unProg = u));
  }

  disconnectedCallback() {
    this.unLog?.();
    this.unProg?.();
    super.disconnectedCallback();
  }

  private async load() {
    this.loading = true;
    this.instances = await listInstances();
    this.loading = false;
  }

  private get featured(): InstanceDto | undefined {
    return this.instances[0];
  }

  private get others(): InstanceDto[] {
    return this.instances.slice(1);
  }

  private async onInstall(id: string, version: string) {
    this.busy = true;
    this.showLogs = true;
    this.status = `正在安装 ${version}（含资源，可能较久）…`;
    try {
      await ensureJava(21);
      const msg = await installVersion(version, true);
      showToast(msg);
      this.status = msg;
      await this.load();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }

  private async onLaunch(id: string, version: string, ready?: boolean) {
    if (!ready) {
      await this.onInstall(id, version);
      // reload ready flag
      await this.load();
      const inst = this.instances.find((i) => i.id === id);
      if (!inst?.version_ready) {
        // still try launch — backend checks disk
      }
    }
    this.busy = true;
    this.showLogs = true;
    this.logs = [];
    this.status = "正在启动…";
    try {
      const msg = await launchInstance(id);
      this.status = msg;
      showToast(msg);
      await this.load();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.status = msg;
      showToast(msg);
      this.logs = [...this.logs, msg];
    } finally {
      this.busy = false;
    }
  }

  private async onStop() {
    await stopInstance();
    this.status = "已请求停止";
    showToast("已停止");
  }

  private icons = ["🌲", "🛡️", "🧊", "⛏️"];

  render() {
    const f = this.featured;
    if (this.loading || !f) {
      return html`<p>加载中…</p>`;
    }
    const ver = typeof f.version === "string" ? f.version : String(f.version);
    const ready = !!f.version_ready;
    return html`
      <section class="hero">
        <div class="hero-body">
          <div class="thumb" aria-hidden="true">☀</div>
          <h1>${f.name}</h1>
          <div class="tags">
            <span class="tag">${ver}</span>
            <span class="tag accent">${f.loader}</span>
            ${ready
              ? html`<span class="tag">已安装</span>`
              : html`<span class="tag warn">未安装</span>`}
          </div>
          <p class="desc">${f.description ?? "Java Edition · 离线账户可玩"}</p>
          <div class="meta">
            ${f.last_played ? `上次游玩: ${f.last_played}` : "尚未启动"} · 离线模式可用
          </div>
        </div>
        <div class="actions">
          <button
            class="launch"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onLaunch(f.id, ver, ready)}
          >
            ▶ ${ready ? "启动" : "安装并启动"}
          </button>
          ${!ready
            ? html`<button
                class="secondary"
                type="button"
                ?disabled=${this.busy}
                @click=${() => this.onInstall(f.id, ver)}
              >
                仅安装 ${ver}
              </button>`
            : html`<button class="secondary" type="button" @click=${() => this.onStop()}>
                停止
              </button>`}
          <button class="secondary" type="button" @click=${() => (this.showLogs = !this.showLogs)}>
            ${this.showLogs ? "隐藏日志" : "显示日志"}
          </button>
        </div>
      </section>

      <p class="status">${this.status}</p>

      ${this.showLogs
        ? html`<div class="logs">
            ${this.logs.length
              ? this.logs.join("\n")
              : html`<span class="logs-empty">日志将显示在这里…</span>`}
          </div>`
        : null}

      <div class="section-head">
        <h2>Your Instances</h2>
        <button class="link" type="button" @click=${() => this.load()}>刷新</button>
      </div>
      <div class="grid">
        ${this.others.map(
          (inst, i) => html`
            <article class="card">
              <div class="card-icon">${this.icons[i % this.icons.length]}</div>
              <h3>${inst.name}</h3>
              <div class="ver">${inst.version}</div>
              <div class="status-line ${inst.version_ready ? "ready" : "need"}">
                <span class="dot"></span>
                ${inst.version_ready ? "Ready" : "需安装"}
              </div>
            </article>
          `,
        )}
        ${this.others.length === 0
          ? html`<article class="card">
              <h3>提示</h3>
              <div class="ver">可用 CLI 创建更多实例：easymc create-instance</div>
            </article>`
          : null}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-home-page": HomePage;
  }
}
