import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import type { InstanceDto, VersionSummary } from "../types";
import { versionId, versionType } from "../types";
import {
  listInstancesState,
  launchInstance,
  stopInstance,
  installVersion,
  ensureJava,
  listenLogs,
  listenInstallProgress,
  setActiveInstance,
  createInstance,
  listVersions,
  listLoaderVersions,
  installLoaderVersion,
  exportInstance,
  importInstance,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import {
  formatProgressLine,
  overallPercent,
  stageLabel,
  stagePercent,
} from "../install-progress";
import { loaderLabel } from "../types";
import { showToast } from "../toast";

@customElement("easymc-home-page")
export class HomePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private loading = true;
  @state() private busy = false;
  @state() private status = "";
  @state() private logs: string[] = [];
  @state() private showLogs = false;

  @state() private showCreate = false;
  @state() private newName = "";
  @state() private newVersion = "1.21.1";
  @state() private includeSnapshots = false;
  @state() private versions: VersionSummary[] = [];
  @state() private versionsLoading = false;
  @state() private versionFilter = "";
  @state() private alsoInstall = true;
  @state() private withAssets = true;
  @state() private newLoader: "vanilla" | "fabric" | "quilt" | "forge" | "neoforge" = "vanilla";
  @state() private loaderVersions: { version: string; stable: boolean; intermediary: string }[] = [];
  @state() private newLoaderVersion = "";
  @state() private progress: InstallProgressDto | null = null;

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
    .launch {
      transition:
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-press) var(--easymc-ease-out),
        opacity var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .launch:hover:not(:disabled) {
      box-shadow: var(--md-sys-elevation-3);
    }
    .launch:active:not(:disabled) {
      transform: scale(0.98);
    }
    .secondary {
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface);
      transition:
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        background var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .secondary:active:not(:disabled) {
      transform: scale(0.98);
    }
    .hero {
      animation: hero-in var(--easymc-dur-page) var(--easymc-ease-out) both;
    }
    @keyframes hero-in {
      from { opacity: 0; transform: translateY(8px); }
      to { opacity: 1; transform: none; }
    }
    @media (prefers-reduced-motion: reduce) {
      .hero { animation: none; }
      .launch:active:not(:disabled),
      .secondary:active:not(:disabled) { transform: none; }
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
      gap: 12px;
      flex-wrap: wrap;
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
      border: 2px solid transparent;
      cursor: pointer;
      text-align: left;
      font: inherit;
      color: inherit;
    }
    .card:hover {
      background: var(--md-sys-color-surface-container-high);
    }
    .card.active {
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
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
    .progress-card {
      margin: 0 0 16px;
      padding: 14px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
    }
    .progress-card.error {
      border: 1px solid #ef9a9a;
      background: #ffebee;
    }
    .progress-head {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: baseline;
      margin-bottom: 8px;
      font-size: 0.9rem;
      font-weight: 600;
    }
    .progress-head .muted {
      font-weight: 500;
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.8rem;
    }
    .bar {
      height: 8px;
      border-radius: 999px;
      background: var(--md-sys-color-surface-container-highest);
      overflow: hidden;
    }
    .bar > i {
      display: block;
      height: 100%;
      background: linear-gradient(90deg, #4db6ac, #0f766e);
      border-radius: 999px;
      transition: width 160ms ease;
    }
    .bar.error > i {
      background: #c62828;
    }
    .progress-detail {
      margin-top: 8px;
      font-size: 0.8rem;
      color: var(--md-sys-color-on-surface-variant);
      word-break: break-all;
    }
    .panel {
      margin-bottom: 20px;
      padding: 20px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    .panel h2 {
      margin: 0;
      font-size: 1.1rem;
    }
    label {
      display: flex;
      flex-direction: column;
      gap: 6px;
      font-size: 0.85rem;
      font-weight: 500;
      color: var(--md-sys-color-on-surface-variant);
    }
    input[type="text"] {
      padding: 10px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      background: var(--md-sys-color-surface);
    }
    .toolbar {
      display: flex;
      gap: 10px;
      flex-wrap: wrap;
      align-items: center;
    }
    .check {
      display: inline-flex;
      flex-direction: row;
      align-items: center;
      gap: 8px;
      font-size: 0.85rem;
      cursor: pointer;
    }
    .ver-list {
      max-height: 200px;
      overflow: auto;
      border: 1px solid var(--md-sys-color-outline-variant);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface);
    }
    .ver-item {
      display: flex;
      justify-content: space-between;
      width: 100%;
      padding: 10px 14px;
      border: none;
      border-bottom: 1px solid var(--md-sys-color-outline-variant);
      background: transparent;
      text-align: left;
      cursor: pointer;
      font: inherit;
    }
    .ver-item:hover {
      background: var(--md-sys-color-surface-container);
    }
    .ver-item.selected {
      background: var(--md-sys-color-primary-container);
    }
    .badge {
      font-size: 0.72rem;
      padding: 2px 8px;
      border-radius: 999px;
      background: #c8e6c9;
      color: #1b5e20;
    }
    .row {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }

    .loader-row {
      display: flex;
      flex-direction: column;
      gap: 8px;
      margin-bottom: 12px;
    }
    .field-label {
      font-size: 0.85rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .chip-row {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }
    .chip {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
      border-radius: 999px;
      padding: 6px 14px;
      font: inherit;
      cursor: pointer;
    }
    .chip.on {
      background: #0f766e;
      color: #fff;
      border-color: #0f766e;
    }
    .field {
      display: flex;
      flex-direction: column;
      gap: 6px;
      margin-bottom: 12px;
      font-size: 0.85rem;
    }
    .field select {
      font: inherit;
      padding: 8px 10px;
      border-radius: 8px;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface);
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
      this.progress = p;
      this.status = formatProgressLine(p);
      if (p.stage === "error") {
        this.logs = [...this.logs.slice(-400), formatProgressLine(p)];
        this.showLogs = true;
      }
    }).then((u) => (this.unProg = u));
  }

  disconnectedCallback() {
    this.unLog?.();
    this.unProg?.();
    super.disconnectedCallback();
  }

  private async load() {
    this.loading = true;
    const state = await listInstancesState();
    this.instances = state.instances;
    this.activeId = state.active_id;
    this.loading = false;
  }

  private get featured(): InstanceDto | undefined {
    return (
      this.instances.find((i) => i.id === this.activeId) ?? this.instances[0]
    );
  }

  private async onInstall(id: string, version: string, withAssets = this.withAssets) {
    this.busy = true;
    this.showLogs = true;
    this.progress = null;
    const mode = withAssets ? "含完整资源，可能较久" : "跳过资源（--no-assets）";
    this.status = `正在安装 ${version}（${mode}）…`;
    try {
      await ensureJava(21);
      const msg = await installVersion(version, withAssets);
      showToast(msg);
      this.status = msg;
      await this.load();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.status = msg;
      this.logs = [...this.logs, msg];
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }

  private async onLaunch(id: string, version: string, ready?: boolean) {
    if (!ready) {
      await this.onInstall(id, version);
      await this.load();
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

  private async onSelect(id: string) {
    try {
      const state = await setActiveInstance(id);
      this.instances = state.instances;
      this.activeId = state.active_id;
    } catch (e) {
      showToast(String(e));
    }
  }

  private async openCreate() {
    this.showCreate = true;
    this.newName = "";
    this.newVersion = "1.21.1";
    this.newLoader = "vanilla";
    this.newLoaderVersion = "";
    this.loaderVersions = [];
    this.versionFilter = "";
    this.versionsLoading = true;
    try {
      this.versions = await listVersions(this.includeSnapshots);
      if (this.versions[0] && !this.versions.find((v) => v.id === this.newVersion)) {
        this.newVersion = this.versions[0].id;
      }
    } finally {
      this.versionsLoading = false;
    }
  }

  private filteredVersions(): VersionSummary[] {
    const q = this.versionFilter.trim().toLowerCase();
    if (!q) return this.versions;
    return this.versions.filter(
      (v) => v.id.toLowerCase().includes(q) || versionType(v).toLowerCase().includes(q),
    );
  }


  private async refreshLoaderVersions() {
    if (this.newLoader === "vanilla") {
      this.loaderVersions = [];
      this.newLoaderVersion = "";
      return;
    }
    try {
      const list = await listLoaderVersions(this.newLoader, this.newVersion);
      this.loaderVersions = list;
      const stable = list.find((l) => l.stable);
      this.newLoaderVersion = (stable ?? list[0])?.version ?? "";
    } catch (e) {
      this.loaderVersions = [];
      this.newLoaderVersion = "";
      const msg = e instanceof Error ? e.message : String(e);
      showToast(msg);
    }
  }

  private async onLoaderChange(loader: "vanilla" | "fabric" | "quilt" | "forge" | "neoforge") {
    this.newLoader = loader;
    await this.refreshLoaderVersions();
  }

  private async onCreate() {
    const name = this.newName.trim() || `实例 ${this.newVersion}`;
    this.busy = true;
    try {
      const inst = await createInstance(name, this.newVersion, this.newLoader, this.newLoaderVersion || null);
      showToast(`已创建 ${inst.name}`);
      if (this.alsoInstall && !inst.version_ready) {
        this.showLogs = true;
        this.progress = null;
        const loaderTip =
          this.newLoader === "vanilla" ? "" : ` · ${loaderLabel(this.newLoader)}`;
        this.status = `正在安装 ${this.newVersion}${loaderTip}…`;
        await ensureJava(21);
        const msg =
          this.newLoader === "vanilla"
            ? await installVersion(this.newVersion, this.withAssets)
            : await installLoaderVersion(
                this.newVersion,
                this.newLoader,
                this.newLoaderVersion || null,
                this.withAssets,
              );
        showToast(msg);
        this.status = msg;
      } else if (this.alsoInstall && inst.version_ready) {
        this.status = `${loaderLabel(this.newLoader)} ${this.newVersion} 已在本地，跳过下载`;
        showToast(this.status);
      }
      this.showCreate = false;
      await this.load();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      showToast(msg);
      this.status = msg;
    } finally {
      this.busy = false;
    }
  }


  private async onExportShare(pack: boolean) {
    const f = this.featured;
    if (!f) return;
    this.busy = true;
    this.status = pack ? "正在导出整合包…" : "正在导出元数据…";
    try {
      const res = await exportInstance(f.id, pack);
      this.status = res.message_zh;
      showToast(res.message_zh.slice(0, 80));
      this.logs = [...this.logs.slice(-400), res.path];
      try {
        await navigator.clipboard.writeText(res.path);
        showToast("路径已复制到剪贴板");
      } catch {
        /* clipboard may be unavailable */
      }
    } catch (e) {
      const msg = String(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }

  private async onImportShare() {
    const path = window.prompt("导入分享包路径（.json / .zip）", "");
    if (!path?.trim()) return;
    const applyMem = window.confirm("将分享包内存建议写入 settings？");
    this.busy = true;
    this.status = "正在导入…";
    try {
      const res = await importInstance(path.trim(), { applyMemory: applyMem });
      this.status = res.message_zh;
      showToast(res.message_zh.slice(0, 80));
      const st = await listInstancesState();
      this.instances = st.instances;
      this.activeId = st.active_id;
    } catch (e) {
      const msg = String(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }

  private icons = ["🌲", "🛡️", "🧊", "⛏️", "🏰", "🌊"];

  render() {
    if (this.loading) {
      return html`<p>加载中…</p>`;
    }
    const f = this.featured;
    if (!f) {
      return html`
        <p>还没有实例。</p>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button class="launch" type="button" @click=${() => this.openCreate()}>新建实例</button>
          <button class="secondary" type="button" ?disabled=${this.busy} @click=${() => this.onImportShare()}>
            导入分享
          </button>
        </div>
        ${this.showCreate ? this.renderCreate() : null}
      `;
    }
    const ver = versionId(f.version);
    const ready = !!f.version_ready;
    return html`
      <section class="hero">
        <div class="hero-body">
          <div class="thumb" aria-hidden="true">☀</div>
          <h1>${f.name}</h1>
          <div class="tags">
            <span class="tag">${ver}</span>
            <span class="tag accent">${loaderLabel(f.loader)}${f.loader_version ? ` ${f.loader_version}` : ""}</span>
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
            ? html`
                <button
                  class="secondary"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.onInstall(f.id, ver, true)}
                >
                  仅安装 ${ver}
                </button>
                <button
                  class="secondary"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.onInstall(f.id, ver, false)}
                >
                  快速安装（无资源）
                </button>
              `
            : html`<button class="secondary" type="button" @click=${() => this.onStop()}>
                停止
              </button>`}
          <button class="secondary" type="button" @click=${() => this.openCreate()}>
            新建实例
          </button>
          <button
            class="secondary"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onExportShare(false)}
          >
            导出元数据
          </button>
          <button
            class="secondary"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onExportShare(true)}
          >
            导出整合包
          </button>
          <button
            class="secondary"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onImportShare()}
          >
            导入分享
          </button>
          <button class="secondary" type="button" @click=${() => (this.showLogs = !this.showLogs)}>
            ${this.showLogs ? "隐藏日志" : "显示日志"}
          </button>
        </div>
      </section>

      <p class="status">${this.status}</p>
      ${this.renderProgress()}

      ${this.showCreate ? this.renderCreate() : null}

      ${this.showLogs
        ? html`<div class="logs">
            ${this.logs.length
              ? this.logs.join("\n")
              : html`<span class="logs-empty">日志将显示在这里…</span>`}
          </div>`
        : null}

      <div class="section-head">
        <h2>你的实例</h2>
        <div>
          <button class="link" type="button" @click=${() => this.openCreate()}>新建</button>
          ·
          <button class="link" type="button" @click=${() => this.load()}>刷新</button>
        </div>
      </div>
      <div class="grid">
        ${this.instances.map(
          (inst, i) => html`
            <button
              class="card ${inst.id === this.activeId ? "active" : ""}"
              type="button"
              @click=${() => this.onSelect(inst.id)}
            >
              <div class="card-icon">${this.icons[i % this.icons.length]}</div>
              <h3>${inst.name}</h3>
              <div class="ver">${versionId(inst.version)}</div>
              <div class="status-line ${inst.version_ready ? "ready" : "need"}">
                <span class="dot"></span>
                ${inst.id === this.activeId
                  ? "当前默认"
                  : inst.version_ready
                    ? "就绪"
                    : "需安装"}
              </div>
            </button>
          `,
        )}
      </div>
    `;
  }

  private renderProgress() {
    const p = this.progress;
    if (!p) return null;
    if (!this.busy && p.stage !== "error" && p.stage !== "done") return null;
    const overall = overallPercent(p);
    const local = stagePercent(p);
    const width = overall ?? local ?? 0;
    const isErr = p.stage === "error";
    const counts =
      (p.skipped ?? 0) > 0 || (p.downloaded ?? 0) > 0
        ? `新下 ${p.downloaded ?? 0} · 跳过 ${p.skipped ?? 0}`
        : "";
    return html`
      <div class="progress-card ${isErr ? "error" : ""}">
        <div class="progress-head">
          <span
            >${p.phase != null && p.phase_total != null
              ? html`${p.phase}/${p.phase_total} · `
              : null}${stageLabel(p)}</span
          >
          <span class="muted"
            >${overall != null ? `${overall.toFixed(0)}% 总进度` : local != null ? `${local.toFixed(0)}%` : ""}
            ${counts ? ` · ${counts}` : ""}</span
          >
        </div>
        <div class="bar ${isErr ? "error" : ""}"><i style="width:${width}%"></i></div>
        <div class="progress-detail">${p.detail}</div>
      </div>
    `;
  }

  private renderCreate() {
    const list = this.filteredVersions();
    return html`
      <section class="panel">
        <h2>新建实例</h2>
        <label>
          名称
          <input
            type="text"
            .value=${this.newName}
            placeholder="例如：速通 1.20"
            @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)}
          />
        </label>
        <div class="toolbar">
          <label style="flex:1;min-width:140px">
            筛选
            <input
              type="text"
              .value=${this.versionFilter}
              placeholder="搜索版本…"
              @input=${(e: Event) => (this.versionFilter = (e.target as HTMLInputElement).value)}
            />
          </label>
          <label class="check">
            <input
              type="checkbox"
              .checked=${this.includeSnapshots}
              @change=${async (e: Event) => {
                this.includeSnapshots = (e.target as HTMLInputElement).checked;
                this.versionsLoading = true;
                this.versions = await listVersions(this.includeSnapshots);
                this.versionsLoading = false;
              }}
            />
            显示快照
          </label>
          
          <div class="loader-row">
            <span class="field-label">加载器</span>
            <div class="chip-row">
              ${(["vanilla", "fabric", "quilt", "forge", "neoforge"] as const).map(
                (l) => html`
                  <button
                    type="button"
                    class="chip ${this.newLoader === l ? "on" : ""}"
                    ?disabled=${this.busy}
                    @click=${() => this.onLoaderChange(l)}
                  >
                    ${loaderLabel(l)}
                  </button>
                `,
              )}
            </div>
          </div>
          ${this.newLoader !== "vanilla"
            ? html`
                <label class="field">
                  <span>Loader 版本</span>
                  <select
                    .value=${this.newLoaderVersion}
                    ?disabled=${this.busy || this.loaderVersions.length === 0}
                    @change=${(e: Event) =>
                      (this.newLoaderVersion = (e.target as HTMLSelectElement).value)}
                  >
                    ${this.loaderVersions.length === 0
                      ? html`<option value="">加载中 / 无可用版本</option>`
                      : this.loaderVersions.map(
                          (l) => html`
                            <option value=${l.version}>
                              ${l.version}${l.stable ? "（稳定）" : ""}
                            </option>
                          `,
                        )}
                  </select>
                </label>
              `
            : null}
<label class="check">
            <input
              type="checkbox"
              .checked=${this.alsoInstall}
              @change=${(e: Event) => (this.alsoInstall = (e.target as HTMLInputElement).checked)}
            />
            创建后安装
          </label>
          <label class="check">
            <input
              type="checkbox"
              .checked=${this.withAssets}
              @change=${(e: Event) => (this.withAssets = (e.target as HTMLInputElement).checked)}
            />
            下载游戏资源
          </label>
        </div>
        <div class="ver">
          已选：<strong>${this.newVersion}</strong>
          ${this.versionsLoading ? " · 加载中…" : ""}
        </div>
        <div class="ver-list">
          ${list.map(
            (v) => html`
              <button
                type="button"
                class="ver-item ${v.id === this.newVersion ? "selected" : ""}"
                @click=${() => { this.newVersion = v.id; void this.refreshLoaderVersions(); }}
              >
                <span>${v.id}</span>
                <span>
                  ${versionType(v)}
                  ${v.installed ? html`<span class="badge">已安装</span>` : null}
                </span>
              </button>
            `,
          )}
        </div>
        <div class="row">
          <button class="launch" type="button" ?disabled=${this.busy} @click=${() => this.onCreate()}>
            创建
          </button>
          <button class="secondary" type="button" @click=${() => (this.showCreate = false)}>取消</button>
        </div>
      </section>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-home-page": HomePage;
  }
}
