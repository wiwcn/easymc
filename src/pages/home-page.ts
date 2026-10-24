import { LitElement, css, html, nothing } from "lit";
import { customElement, state } from "lit/decorators.js";
import {
  createInstance,
  getLogs,
  installLoaderVersion,
  installVersion,
  launchInstance,
  listAccounts,
  listInstancesState,
  listLoaderVersions,
  listVersions,
  listenInstallProgress,
  listenLogs,
  setActiveInstance,
  stopInstance,
  writeLaunchScript,
  openInstanceFolder,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import type { AccountDto, InstanceDto, LoaderId, VersionSummary } from "../types";
import { loaderLabel, versionId, versionType } from "../types";
import { showToast } from "../toast";

@customElement("easymc-home-page")
export class HomePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private accounts: AccountDto[] = [];
  @state() private loading = true;
  @state() private busy = false;
  @state() private status = "";
  @state() private progress: InstallProgressDto | null = null;
  @state() private showLogs = false;
  @state() private logs: string[] = [];
  @state() private menuOpen = false;
  @state() private showCreate = false;
  @state() private newName = "";
  @state() private newVersion = "1.21.1";
  @state() private includeSnapshots = false;
  @state() private versions: VersionSummary[] = [];
  @state() private versionsLoading = false;
  @state() private versionFilter = "";
  @state() private alsoInstall = true;
  @state() private withAssets = true;
  @state() private newLoader: LoaderId = "vanilla";
  @state() private loaderVersions: { version: string; stable: boolean; intermediary: string }[] = [];
  @state() private newLoaderVersion = "";

  private unlog?: () => void;
  private unprog?: () => void;

  static styles = css`
    :host { display: block; max-width: 720px; }
    h1 {
      font-size: 1.35rem;
      font-weight: 650;
      letter-spacing: -0.02em;
      margin: 8px 0 4px;
      color: var(--md-sys-color-on-surface);
    }
    .sub {
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.9rem;
      margin-bottom: 20px;
    }
    .launch-row {
      display: flex;
      gap: 10px;
      align-items: stretch;
      flex-wrap: wrap;
    }
    button.play {
      flex: 1 1 280px;
      min-height: 64px;
      border: none;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      cursor: pointer;
      text-align: left;
      padding: 12px 20px;
      display: flex;
      flex-direction: column;
      justify-content: center;
      gap: 2px;
      box-shadow: var(--md-sys-elevation-1);
      transition: transform var(--easymc-dur-press) var(--easymc-ease-out),
        filter var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.play:hover { filter: brightness(1.05); }
    button.play:active { transform: scale(0.985); }
    button.play:disabled { opacity: 0.55; cursor: not-allowed; transform: none; }
    button.play .t1 { font-size: 1.1rem; font-weight: 650; }
    button.play .t2 { font-size: 0.78rem; opacity: 0.9; }
    .side {
      display: flex;
      gap: 8px;
      align-items: stretch;
    }
    select.ver, button.ghost, button.menu {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 0 16px;
      min-height: 40px;
      font-size: 0.9rem;
      cursor: pointer;
    }
    select.ver { min-width: 160px; align-self: center; }
    button.menu { width: 40px; padding: 0; font-size: 1.2rem; line-height: 1; position: relative; }
    .menu-panel {
      position: absolute;
      right: 0;
      top: calc(100% + 6px);
      z-index: 20;
      min-width: 200px;
      background: var(--md-sys-color-surface-container-lowest);
      border: 1px solid var(--md-sys-color-outline-variant);
      border-radius: var(--md-sys-shape-corner-medium);
      box-shadow: var(--md-sys-elevation-2, 0 4px 12px rgba(0,0,0,.12));
      padding: 6px;
      display: flex;
      flex-direction: column;
    }
    .menu-panel button {
      border: none;
      background: transparent;
      text-align: left;
      padding: 10px 12px;
      border-radius: var(--md-sys-shape-corner-small);
      cursor: pointer;
      color: var(--md-sys-color-on-surface);
      font-size: 0.9rem;
    }
    .menu-panel button:hover { background: var(--md-sys-color-surface-container); }
    .menu-wrap { position: relative; align-self: center; }
    .card {
      margin-top: 18px;
      padding: 14px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid color-mix(in srgb, var(--md-sys-color-outline-variant) 70%, transparent);
    }
    .card h2 { margin: 0 0 8px; font-size: 0.95rem; }
    .row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
    .muted { color: var(--md-sys-color-on-surface-variant); font-size: 0.85rem; }
    .status { margin-top: 12px; font-size: 0.9rem; color: var(--md-sys-color-on-surface-variant); }
    .bar {
      height: 6px; border-radius: 999px; background: var(--md-sys-color-surface-container-high);
      overflow: hidden; margin-top: 8px;
    }
    .bar > i {
      display: block; height: 100%; background: var(--md-sys-color-primary);
      width: var(--w, 0%); transition: width var(--easymc-dur-press) var(--easymc-ease-out);
    }
    pre.logs {
      max-height: 180px; overflow: auto; font-size: 0.75rem;
      background: var(--md-sys-color-surface-container); padding: 10px; border-radius: 8px;
    }
    .create {
      margin-top: 16px; padding: 16px; border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      border: 1px solid var(--md-sys-color-outline-variant);
    }
    input, select {
      border: 1px solid var(--md-sys-color-outline-variant);
      border-radius: var(--md-sys-shape-corner-small);
      padding: 8px 10px; background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
    }
    button.primary {
      border: none; background: var(--md-sys-color-primary); color: var(--md-sys-color-on-primary);
      border-radius: var(--md-sys-shape-corner-full); padding: 8px 16px; cursor: pointer;
    }
    button.ghost {
      background: transparent;
    }
    @media (prefers-reduced-motion: reduce) {
      button.play:active { transform: none; }
      .bar > i { transition: none; }
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.boot();
    void listenLogs((l) => {
      this.logs = [...this.logs.slice(-200), l];
    }).then((u) => (this.unlog = u));
    void listenInstallProgress((p) => (this.progress = p)).then((u) => (this.unprog = u));
    window.addEventListener("click", this.onDocClick);
  }
  disconnectedCallback() {
    this.unlog?.();
    this.unprog?.();
    window.removeEventListener("click", this.onDocClick);
    super.disconnectedCallback();
  }
  private onDocClick = () => {
    if (this.menuOpen) this.menuOpen = false;
  };

  private get current(): InstanceDto | null {
    return this.instances.find((i) => i.id === this.activeId) ?? this.instances[0] ?? null;
  }
  private get account(): AccountDto | null {
    const cur = this.current;
    if (cur?.account_id) {
      return this.accounts.find((a) => a.id === cur.account_id) ?? this.accounts[0] ?? null;
    }
    return this.accounts[0] ?? null;
  }

  private async boot() {
    this.loading = true;
    try {
      const [st, acc] = await Promise.all([listInstancesState(), listAccounts()]);
      this.instances = st.instances;
      this.activeId = st.active_id;
      this.accounts = acc;
      this.logs = await getLogs();
    } finally {
      this.loading = false;
    }
  }

  private async onSelect(id: string) {
    const st = await setActiveInstance(id);
    this.instances = st.instances;
    this.activeId = st.active_id;
  }

  private async onPlay() {
    const f = this.current;
    if (!f) return;
    this.busy = true;
    this.status = "";
    this.progress = null;
    try {
      const ver = versionId(f.version);
      if (!f.version_ready) {
        this.status = "正在准备游戏文件…";
        if (f.loader === "vanilla" || !f.loader) {
          await installVersion(ver, true);
        } else {
          await installLoaderVersion(ver, f.loader, f.loader_version ?? null, true);
        }
      }
      this.status = "正在启动游戏…";
      const msg = await launchInstance(f.id);
      this.status = msg;
      showToast("游戏已启动");
      await this.boot();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
      this.progress = null;
    }
  }

  private go(nav: string) {
    this.dispatchEvent(new CustomEvent("nav-change", { detail: nav, bubbles: true, composed: true }));
  }

  private async openCreate() {
    this.showCreate = true;
    this.versionsLoading = true;
    try {
      this.versions = await listVersions(this.includeSnapshots);
    } finally {
      this.versionsLoading = false;
    }
  }

  private async refreshLoaderVersions() {
    if (this.newLoader === "vanilla") {
      this.loaderVersions = [];
      this.newLoaderVersion = "";
      return;
    }
    this.loaderVersions = await listLoaderVersions(this.newLoader, this.newVersion);
    this.newLoaderVersion = this.loaderVersions.find((v) => v.stable)?.version ?? this.loaderVersions[0]?.version ?? "";
  }

  private async onCreate() {
    if (!this.newVersion.trim()) {
      showToast("请选择游戏版本");
      return;
    }
    this.busy = true;
    try {
      const name = this.newName.trim() || `我的世界 ${this.newVersion}`;
      const inst = await createInstance(
        name,
        this.newVersion,
        this.newLoader,
        this.newLoader === "vanilla" ? null : this.newLoaderVersion || null,
      );
      if (this.alsoInstall) {
        if (this.newLoader === "vanilla") await installVersion(this.newVersion, this.withAssets);
        else await installLoaderVersion(this.newVersion, this.newLoader, this.newLoaderVersion || null, this.withAssets);
      }
      await this.onSelect(inst.id);
      this.showCreate = false;
      showToast(`已添加「${inst.name}」`);
      await this.boot();
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.busy = false;
    }
  }

  render() {
    if (this.loading) return html`<p class="muted">加载中…</p>`;
    const f = this.current;
    const acc = this.account;
    if (!f) {
      return html`
        <h1>还没有可玩的版本</h1>
        <p class="sub">先添加一个游戏版本，就可以启动游戏了。</p>
        <div class="row">
          <button class="primary" type="button" @click=${() => this.openCreate()}>添加版本</button>
          <button class="ghost" type="button" @click=${() => this.go("download")}>去下载中心</button>
        </div>
        ${this.showCreate ? this.renderCreate() : nothing}
      `;
    }
    const ver = versionId(f.version);
    const ready = !!f.version_ready;
    const playLabel = ready ? "启动游戏" : "安装并启动";
    const sub = `${f.name} · ${loaderLabel(f.loader)} ${ver}${acc ? ` · ${acc.username}` : ""}`;
    const pct = this.progress?.percent;
    return html`
      <h1>准备好了就开玩</h1>
      <p class="sub">选好版本和账户，点下面按钮进入 Minecraft。</p>

      <div class="launch-row">
        <button class="play" type="button" ?disabled=${this.busy} @click=${() => this.onPlay()}>
          <span class="t1">${this.busy ? "请稍候…" : playLabel}</span>
          <span class="t2">${sub}</span>
        </button>
        <div class="side">
          <select
            class="ver"
            aria-label="切换版本"
            .value=${f.id}
            @change=${(e: Event) => this.onSelect((e.target as HTMLSelectElement).value)}
          >
            ${this.instances.map((i) => html`<option value=${i.id}>${i.name}</option>`)}
          </select>
          <div class="menu-wrap">
            <button
              class="menu"
              type="button"
              title="更多操作"
              aria-haspopup="menu"
              @click=${(e: Event) => {
                e.stopPropagation();
                this.menuOpen = !this.menuOpen;
              }}
            >
              ⋮
            </button>
            ${this.menuOpen
              ? html`<div class="menu-panel" role="menu" @click=${(e: Event) => e.stopPropagation()}>
                  <button type="button" @click=${() => { this.menuOpen = false; this.openCreate(); }}>添加版本</button>
                  <button type="button" @click=${() => { this.menuOpen = false; this.go("download"); }}>去下载内容</button>
                  <button type="button" @click=${() => { this.menuOpen = false; this.go("instances"); }}>管理此版本</button>
                  <button type="button" ?disabled=${this.busy} @click=${async () => {
                    this.menuOpen = false;
                    try {
                      const r = await writeLaunchScript(f.id);
                      showToast(r.message_zh);
                    } catch (e) { showToast(e instanceof Error ? e.message : String(e)); }
                  }}>生成启动脚本</button>
                  <button type="button" @click=${async () => {
                    this.menuOpen = false;
                    try { showToast(await openInstanceFolder(f.id, "game")); }
                    catch (e) { showToast(e instanceof Error ? e.message : String(e)); }
                  }}>打开游戏文件夹</button>
                  <button type="button" @click=${() => { this.menuOpen = false; this.showLogs = !this.showLogs; }}>
                    ${this.showLogs ? "隐藏日志" : "查看日志"}
                  </button>
                  <button type="button" @click=${() => { this.menuOpen = false; void stopInstance(); showToast("已请求停止"); }}>
                    停止游戏
                  </button>
                </div>`
              : nothing}
          </div>
        </div>
      </div>

      ${this.status || this.progress
        ? html`<div class="status">
            ${this.progress
              ? html`${this.progress.stage_label} · ${this.progress.detail}
                  <div class="bar"><i style=${`--w:${pct ?? 0}%`}></i></div>`
              : this.status}
          </div>`
        : nothing}

      <div class="card">
        <h2>当前账户</h2>
        ${acc
          ? html`<p class="muted">游戏里显示为 <strong>${acc.username}</strong>（${acc.kind === "microsoft" ? "正版" : "离线"}）</p>
              <button class="ghost" type="button" @click=${() => this.go("settings")}>更换账户</button>`
          : html`<p class="muted">还没有账户。</p>
              <button class="primary" type="button" @click=${() => this.go("settings")}>去添加账户</button>`}
      </div>

      ${this.showLogs
        ? html`<div class="card"><h2>启动日志</h2><pre class="logs">${this.logs.join("\n") || "（暂无）"}</pre></div>`
        : nothing}

      ${this.showCreate ? this.renderCreate() : nothing}
    `;
  }

  private renderCreate() {
    const filtered = this.versions.filter((v) => {
      if (!this.versionFilter.trim()) return true;
      return versionId(v as never).includes?.(this.versionFilter) || v.id.includes(this.versionFilter);
    });
    return html`
      <div class="create">
        <h2 style="margin-top:0">添加版本</h2>
        <div class="row" style="margin-bottom:8px">
          <input placeholder="名字（可选）" .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} />
          <select .value=${this.newLoader} @change=${(e: Event) => { this.newLoader = (e.target as HTMLSelectElement).value as LoaderId; void this.refreshLoaderVersions(); }}>
            <option value="vanilla">原版</option>
            <option value="fabric">Fabric</option>
            <option value="quilt">Quilt</option>
            <option value="forge">Forge</option>
            <option value="neoforge">NeoForge</option>
          </select>
        </div>
        <div class="row" style="margin-bottom:8px">
          <input placeholder="筛选版本…" .value=${this.versionFilter} @input=${(e: Event) => (this.versionFilter = (e.target as HTMLInputElement).value)} />
          <label class="muted"><input type="checkbox" .checked=${this.includeSnapshots} @change=${async (e: Event) => { this.includeSnapshots = (e.target as HTMLInputElement).checked; this.versions = await listVersions(this.includeSnapshots); }} /> 含快照</label>
        </div>
        <select style="width:100%;margin-bottom:8px" .value=${this.newVersion} @change=${(e: Event) => { this.newVersion = (e.target as HTMLSelectElement).value; void this.refreshLoaderVersions(); }}>
          ${this.versionsLoading
            ? html`<option>加载中…</option>`
            : filtered.slice(0, 80).map((v) => html`<option value=${v.id}>${v.id} · ${versionType(v)}</option>`)}
        </select>
        ${this.newLoader !== "vanilla"
          ? html`<select style="width:100%;margin-bottom:8px" .value=${this.newLoaderVersion} @change=${(e: Event) => (this.newLoaderVersion = (e.target as HTMLSelectElement).value)}>
              ${this.loaderVersions.map((v) => html`<option value=${v.version}>${v.version}${v.stable ? "（稳定）" : ""}</option>`)}
            </select>`
          : nothing}
        <label class="muted"><input type="checkbox" .checked=${this.alsoInstall} @change=${(e: Event) => (this.alsoInstall = (e.target as HTMLInputElement).checked)} /> 添加后立刻安装</label>
        <div class="row" style="margin-top:10px">
          <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.onCreate()}>添加</button>
          <button class="ghost" type="button" @click=${() => (this.showCreate = false)}>取消</button>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-home-page": HomePage;
  }
}
