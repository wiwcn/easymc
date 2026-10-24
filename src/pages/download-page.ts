import { LitElement, css, html, nothing } from "lit";
import { customElement, state } from "lit/decorators.js";
import {
  createInstance,
  curseforgeConfigured,
  installContent,
  installLoaderVersion,
  installMod,
  installModpack,
  installVersion,
  listInstancesState,
  listLoaderVersions,
  listModVersions,
  listVersions,
  listenInstallProgress,
  listLocalContent,
  searchContent,
  searchMods,
  setActiveInstance,
  setCurseforgeKey,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import type {
  CurseForgeStatusDto,
  InstanceDto,
  LoaderId,
  ModProjectDto,
  ModSourceId,
  ModVersionDto,
  ProjectTypeId,
  VersionSummary,
} from "../types";
import { loaderLabel, versionType } from "../types";
import { showToast } from "../toast";
import "../components/empty-state";

type DlTab = "vanilla" | "mod" | "modpack" | "resourcepack" | "shader" | "datapack";

@customElement("easymc-download-page")
export class DownloadPage extends LitElement {
  @state() private tab: DlTab = "vanilla";
  @state() private source: ModSourceId = "modrinth";
  @state() private query = "";
  @state() private results: ModProjectDto[] = [];
  @state() private loading = false;
  @state() private error = "";
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private cfKeyDraft = "";
  @state() private progress: InstallProgressDto | null = null;
  @state() private installingId: string | null = null;
  @state() private expandedId: string | null = null;
  @state() private versions: ModVersionDto[] = [];
  @state() private versionsLoading = false;
  @state() private worldPick = "";
  @state() private worlds: string[] = [];

  // vanilla tab
  @state() private mcVersions: VersionSummary[] = [];
  @state() private snapshots = false;
  @state() private filter = "";
  @state() private pick = "1.21.1";
  @state() private loader: LoaderId = "vanilla";
  @state() private loaderVers: { version: string; stable: boolean }[] = [];
  @state() private loaderVer = "";
  @state() private newName = "";
  @state() private busy = false;
  /** Vanilla install wizard: pick MC version → pick loader. */
  @state() private vanillaStep: "version" | "loader" = "version";

  private unprog?: () => void;

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      flex: 1 0 auto;
      min-height: 100%;
      max-width: 1100px;
      width: 100%;
      box-sizing: border-box;
    }
    h1 { font-size: 1.35rem; font-weight: 700; margin: 0 0 2px; letter-spacing: -0.03em; }
    .sub { color: var(--md-sys-color-on-surface-variant); font-size: 0.875rem; margin: 0 0 14px; }
    .tabs { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 16px; }
    .tabs button {
      border: none; background: transparent; cursor: pointer;
      padding: 9px 16px; border-radius: 999px;
      color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem; font-weight: 500;
      transition: background var(--easymc-dur-press) var(--easymc-ease-out), color var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .tabs button.on {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container); font-weight: 650;
    }
    .search-row {
      display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin-bottom: 12px;
    }
    .search {
      flex: 1; min-width: 200px; display: flex; align-items: center; gap: 8px;
      padding: 10px 16px; border-radius: 999px;
      background: var(--md-sys-color-surface-container-lowest);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 70%, transparent);
      box-shadow: var(--md-sys-elevation-1);
    }
    .search input {
      flex: 1; border: none; background: transparent; padding: 0; outline: none;
      font-size: 0.95rem; color: var(--md-sys-color-on-surface);
    }
    .search svg { width: 18px; height: 18px; color: var(--md-sys-color-on-surface-variant); flex-shrink: 0; }
    .sources { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
    .sources .lbl { font-size: 0.8rem; color: var(--md-sys-color-on-surface-variant); margin-right: 2px; }
    button.src {
      display: inline-flex; align-items: center; gap: 6px;
      border: 1.5px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface); border-radius: 999px; padding: 8px 14px;
      font-size: 0.85rem; font-weight: 600; cursor: pointer;
    }
    button.src.on {
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
    }
    .dot {
      width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0;
    }
    .dot.mr { background: oklch(0.62 0.14 150); }
    .dot.cf { background: oklch(0.62 0.14 55); }
    .bar {
      display: flex; gap: 8px; flex-wrap: wrap; align-items: center; margin-bottom: 12px;
    }
    input, select, button {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: 999px; padding: 8px 14px; font-size: 0.9rem;
    }
    button.primary { border: none; background: var(--md-sys-color-primary); color: var(--md-sys-color-on-primary); cursor: pointer; font-weight: 600; }
    button.ghost { background: transparent; cursor: pointer; }
    button.outline-dl {
      border: 1.5px solid var(--md-sys-color-primary);
      background: transparent; color: var(--md-sys-color-primary);
      border-radius: 999px; padding: 8px 14px; cursor: pointer; font-weight: 650; font-size: 0.85rem;
    }
    .market-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
      gap: 12px;
    }
    .card {
      display: flex; flex-direction: column; gap: 10px; padding: 14px;
      border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 40%, transparent);
      align-items: stretch; min-height: 0;
    }
    .card-top { display: flex; gap: 12px; align-items: flex-start; }
    .card img, .card .ph {
      width: 48px; height: 48px; border-radius: 12px; object-fit: cover;
      background: var(--md-sys-color-primary-container); flex-shrink: 0;
    }
    .card .body { flex: 1; min-width: 0; }
    .card .name { font-weight: 650; font-size: 0.95rem; letter-spacing: -0.01em; }
    .card .desc {
      font-size: 0.8rem; color: var(--md-sys-color-on-surface-variant); margin-top: 2px;
      display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    }
    .tags { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
    .tag {
      font-size: 0.7rem; font-weight: 600; padding: 2px 8px; border-radius: 999px;
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
    }
    .card-foot {
      display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: auto;
    }
    .dl-stat { font-size: 0.78rem; color: var(--md-sys-color-on-surface-variant); font-variant-numeric: tabular-nums; }
    .muted { color: var(--md-sys-color-on-surface-variant); font-size: 0.85rem; }
    .results-slot {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 220px;
    }
    .vanilla-body {
      display: flex;
      flex-direction: column;
      flex: 1 1 auto;
      min-height: 240px;
    }
    .panel {
      padding: 18px; border-radius: 18px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
    }
    .field { margin-bottom: 10px; display: flex; flex-direction: column; gap: 4px; max-width: 480px; }
    .field label { font-size: 0.85rem; color: var(--md-sys-color-on-surface-variant); }

    .toolbar {
      display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin-bottom: 14px;
    }
    .toolbar .grow { flex: 1; min-width: 160px; }
    .crumb {
      display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
      font-size: 0.9rem; color: var(--md-sys-color-on-surface-variant); margin-bottom: 12px;
    }
    .crumb strong { color: var(--md-sys-color-on-surface); font-weight: 650; }
    .crumb button {
      border: none; background: transparent; color: var(--md-sys-color-primary);
      cursor: pointer; font: inherit; font-weight: 600; padding: 0;
    }
    .ver-grid, .loader-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
      gap: 10px;
    }
    button.ver-card, button.loader-card {
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      display: flex; gap: 12px; align-items: center;
      padding: 14px 16px; border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      border: 1.5px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 50%, transparent);
      box-shadow: var(--md-sys-elevation-1);
      transition: border-color var(--easymc-dur-press) var(--easymc-ease-out),
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.ver-card:hover, button.loader-card:hover {
      background: var(--md-sys-color-surface-container);
      border-color: var(--md-sys-color-primary);
    }
    button.ver-card:active, button.loader-card:active { transform: scale(0.985); }
    button.ver-card .vid {
      font-weight: 700; font-size: 1.05rem; letter-spacing: -0.02em;
    }
    button.ver-card .meta {
      font-size: 0.78rem; color: var(--md-sys-color-on-surface-variant); margin-top: 2px;
    }
    .badge-inst {
      margin-left: auto; font-size: 0.7rem; font-weight: 650;
      padding: 3px 8px; border-radius: 999px;
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container); flex-shrink: 0;
    }
    .loader-ico {
      width: 40px; height: 40px; border-radius: 12px; flex-shrink: 0;
      display: grid; place-items: center; font-weight: 800; font-size: 0.85rem;
      color: white; letter-spacing: -0.04em;
    }
    .loader-ico.vanilla { background: linear-gradient(135deg, oklch(0.62 0.12 145), oklch(0.48 0.1 150)); }
    .loader-ico.fabric { background: linear-gradient(135deg, oklch(0.72 0.14 85), oklch(0.55 0.12 70)); }
    .loader-ico.quilt { background: linear-gradient(135deg, oklch(0.65 0.14 310), oklch(0.5 0.12 300)); }
    .loader-ico.forge { background: linear-gradient(135deg, oklch(0.55 0.08 60), oklch(0.4 0.06 50)); }
    .loader-ico.neoforge { background: linear-gradient(135deg, oklch(0.58 0.14 35), oklch(0.45 0.12 25)); }
    button.loader-card .lname { font-weight: 700; font-size: 0.98rem; }
    button.loader-card .ldesc {
      font-size: 0.78rem; color: var(--md-sys-color-on-surface-variant); margin-top: 2px; line-height: 1.35;
    }
    .loader-panel {
      padding: 16px; border-radius: 18px; margin-top: 14px;
      background: var(--md-sys-color-surface-container-low);
    }
    .loader-panel .row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; margin-top: 10px; }
    button.back {
      border: none; background: transparent; color: var(--md-sys-color-primary);
      cursor: pointer; font: inherit; font-weight: 600; padding: 8px 4px;
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    const saved = sessionStorage.getItem("easymc-download-tab") as DlTab | null;
    if (saved) {
      this.tab = saved;
      sessionStorage.removeItem("easymc-download-tab");
    }
    void this.bootstrap();
    void listenInstallProgress((p) => (this.progress = p)).then((u) => (this.unprog = u));
  }
  disconnectedCallback() {
    this.unprog?.();
    super.disconnectedCallback();
  }

  private get active(): InstanceDto | null {
    return this.instances.find((i) => i.id === this.activeId) ?? this.instances[0] ?? null;
  }

  private async bootstrap() {
    const [st, cf] = await Promise.all([listInstancesState(), curseforgeConfigured()]);
    this.instances = st.instances;
    this.activeId = st.active_id;
    this.cf = cf;
    if (this.tab === "vanilla") await this.loadVanilla();
  }

  private async loadVanilla() {
    this.mcVersions = await listVersions(this.snapshots);
    if (!this.mcVersions.some((v) => v.id === this.pick) && this.mcVersions[0]) {
      this.pick = this.mcVersions[0].id;
    }
  }

  private async setTab(t: DlTab) {
    this.tab = t;
    if (t === "vanilla") this.vanillaStep = "version";
    this.results = [];
    this.error = "";
    this.expandedId = null;
    if (t === "vanilla") await this.loadVanilla();
  }

  private projectType(): ProjectTypeId {
    if (this.tab === "mod") return "mod";
    if (this.tab === "modpack") return "modpack";
    if (this.tab === "resourcepack") return "resourcepack";
    if (this.tab === "shader") return "shader";
    return "datapack";
  }

  private async onSearch() {
    if (this.tab === "vanilla") return;
    this.loading = true;
    this.error = "";
    try {
      if (this.source === "curseforge" && !this.cf?.configured) {
        this.error = "浏览 CurseForge 需要先在设置里粘贴密钥（高级）。";
        this.results = [];
        return;
      }
      const game = this.active ? String(this.active.version) : null;
      const loader = this.tab === "mod" ? this.active?.loader : null;
      this.results =
        this.tab === "mod"
          ? await searchMods(this.source, this.query, game, loader)
          : await searchContent(this.source, this.projectType(), this.query, game, loader);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.results = [];
    } finally {
      this.loading = false;
    }
  }

  private async onExpand(p: ModProjectDto) {
    if (this.expandedId === p.id) {
      this.expandedId = null;
      return;
    }
    this.expandedId = p.id;
    this.versionsLoading = true;
    try {
      const game = this.active ? String(this.active.version) : null;
      this.versions = await listModVersions(this.source, p.id, game, this.tab === "mod" ? this.active?.loader : null);
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
      this.versions = [];
    } finally {
      this.versionsLoading = false;
    }
  }

  private async onInstall(p: ModProjectDto, versionId?: string | null) {
    this.installingId = p.id;
    this.progress = null;
    try {
      if (this.tab === "modpack") {
        const r = await installModpack(this.source, p.id, { versionId });
        showToast(r.message_zh);
        const st = await listInstancesState();
        this.instances = st.instances;
        this.activeId = r.instance_id;
        await setActiveInstance(r.instance_id);
      } else if (this.tab === "mod") {
        if (!this.active) {
          showToast("请先在启动页选好要玩的版本");
          return;
        }
        const r = await installMod(this.source, p.id, versionId, this.active.id);
        showToast(r.skipped ? `已有 ${r.filename}` : `已装入 ${r.project_name}`);
      } else {
        if (!this.active) {
          showToast("请先在启动页选好要玩的版本");
          return;
        }
        if (this.tab === "datapack" && this.worlds.length === 0) {
          this.worlds = (await listLocalContent(this.active.id, "saves")).map((w) => w.name);
        }
        const r = await installContent(this.source, this.projectType(), p.id, {
          versionId,
          instanceId: this.active.id,
          world: this.tab === "datapack" ? this.worldPick || null : null,
        });
        showToast(r.skipped ? `已有 ${r.filename}` : `已下载 ${r.project_name}`);
      }
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.installingId = null;
    }
  }

  render() {
    const tabs: { id: DlTab; label: string }[] = [
      { id: "vanilla", label: "原版" },
      { id: "mod", label: "Mod" },
      { id: "modpack", label: "整合包" },
      { id: "resourcepack", label: "资源包" },
      { id: "shader", label: "光影" },
      { id: "datapack", label: "数据包" },
    ];
    const sub =
      this.tab === "vanilla"
        ? this.vanillaStep === "version"
          ? "选择游戏版本"
          : `为 ${this.pick} 选择加载器`
        : this.tab === "modpack"
          ? "一键装好整套模组包（会新建一个版本）"
          : this.active
            ? `装进当前版本：${this.active.name}`
            : "先在启动页选一个版本，或直接装整合包";
    return html`
      <p class="sub">${sub}</p>
      <div class="tabs">
        ${tabs.map((t) => html`<button class=${this.tab === t.id ? "on" : ""} type="button" @click=${() => this.setTab(t.id)}>${t.label}</button>`)}
      </div>
      ${this.tab === "vanilla" ? this.renderVanilla() : this.renderMarket()}
    `;
  }

  private renderVanilla() {
    const filtered = this.mcVersions.filter((v) => !this.filter || v.id.includes(this.filter));
    if (this.vanillaStep === "loader") {
      const loaders: { id: LoaderId; name: string; desc: string; mark: string }[] = [
        { id: "vanilla", name: "原版", desc: "纯净 Minecraft，不加模组加载器", mark: "MC" },
        { id: "fabric", name: "Fabric", desc: "轻量模组加载器，更新快", mark: "F" },
        { id: "quilt", name: "Quilt", desc: "Fabric 兼容分支", mark: "Q" },
        { id: "forge", name: "Forge", desc: "经典模组加载器，大整合常用", mark: "Fg" },
        { id: "neoforge", name: "NeoForge", desc: "Forge 后续，适合新版本", mark: "NF" },
      ];
      return html`
        <div class="crumb">
          <button type="button" @click=${() => { this.vanillaStep = "version"; this.loader = "vanilla"; this.loaderVer = ""; }}>← 重选版本</button>
          <span>/</span>
          <strong>${this.pick}</strong>
          <span>· 选择加载器</span>
        </div>
        <div class="loader-grid" role="listbox" aria-label="加载器">
          ${loaders.map(
            (l) => html`<button
              class="loader-card"
              type="button"
              role="option"
              @click=${() => this.pickLoader(l.id)}
            >
              <span class="loader-ico ${l.id}" aria-hidden="true">${l.mark}</span>
              <span>
                <div class="lname">${l.name}</div>
                <div class="ldesc">${l.desc}</div>
              </span>
            </button>`,
          )}
        </div>
        ${this.loader !== "vanilla" && this.loaderVers.length
          ? html`<div class="loader-panel">
              <div class="muted">加载器版本（${loaderLabel(this.loader)}）</div>
              <div class="row">
                <select .value=${this.loaderVer} @change=${(e: Event) => (this.loaderVer = (e.target as HTMLSelectElement).value)}>
                  ${this.loaderVers.map((v) => html`<option value=${v.version}>${v.version}${v.stable ? "（稳定）" : ""}</option>`)}
                </select>
                <input .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} placeholder="版本名称（可选）" style="flex:1;min-width:140px" />
                <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.installVanilla()}>
                  ${this.busy ? "安装中…" : "安装并添加版本"}
                </button>
              </div>
            </div>`
          : this.loader === "vanilla"
            ? html`<div class="loader-panel">
                <div class="muted">将安装原版 ${this.pick}</div>
                <div class="row">
                  <input .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} placeholder="版本名称（可选）" style="flex:1;min-width:140px" />
                  <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.installVanilla()}>
                    ${this.busy ? "安装中…" : "安装并添加版本"}
                  </button>
                </div>
              </div>`
            : html`<div class="results-slot"><easymc-empty-state
                  variant="panel"
                  title="咦？这个加载器好像还没版本…"
                  description="换个游戏版本或加载器再试试"
                ></easymc-empty-state></div>`}
        ${this.progress ? html`<p class="muted" style="margin-top:10px">${this.progress.stage_label} · ${this.progress.detail}</p>` : nothing}
      `;
    }

    const typeLabel = (v: VersionSummary) => {
      const ty = versionType(v);
      if (ty === "snapshot") return "快照";
      if (ty === "old_beta" || ty === "old_alpha") return "远古";
      return "正式版";
    };

    return html`
      <div class="vanilla-body">
        <div class="toolbar">
          <div class="search grow">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg>
            <input placeholder="筛选版本号…" .value=${this.filter} @input=${(e: Event) => (this.filter = (e.target as HTMLInputElement).value)} />
          </div>
          <label class="muted" style="display:inline-flex;align-items:center;gap:6px">
            <input type="checkbox" .checked=${this.snapshots} @change=${async (e: Event) => { this.snapshots = (e.target as HTMLInputElement).checked; await this.loadVanilla(); }} />
            含快照
          </label>
        </div>
        ${!filtered.length
          ? html`<div class="results-slot">
              <easymc-empty-state
                variant="panel"
                title="没有符合筛选的版本"
                description="换个关键词试试，或者打开「含快照」看看"
              ></easymc-empty-state>
            </div>`
          : html`<div class="ver-grid" role="listbox" aria-label="游戏版本">
              ${filtered.slice(0, 48).map(
                (v) => html`<button
                  class="ver-card"
                  type="button"
                  role="option"
                  @click=${() => this.pickVersion(v.id)}
                >
                  <span>
                    <div class="vid">${v.id}</div>
                    <div class="meta">${typeLabel(v)}</div>
                  </span>
                  ${v.installed ? html`<span class="badge-inst">已安装</span>` : nothing}
                </button>`,
              )}
            </div>`}
      </div>
    `;
  }

  private async pickVersion(id: string) {
    this.pick = id;
    this.loader = "vanilla";
    this.loaderVer = "";
    this.loaderVers = [];
    this.vanillaStep = "loader";
  }

  private async pickLoader(id: LoaderId) {
    this.loader = id;
    if (id === "vanilla") {
      this.loaderVers = [];
      this.loaderVer = "";
      return;
    }
    this.loaderVers = await listLoaderVersions(id, this.pick);
    this.loaderVer = this.loaderVers[0]?.version ?? "";
  }

  private async installVanilla() {
    this.busy = true;
    try {
      const name = this.newName.trim() || `我的世界 ${this.pick}${this.loader === "vanilla" ? "" : ` (${loaderLabel(this.loader)})`}`;
      const inst = await createInstance(name, this.pick, this.loader, this.loader === "vanilla" ? null : this.loaderVer || null);
      if (this.loader === "vanilla") await installVersion(this.pick, true);
      else await installLoaderVersion(this.pick, this.loader, this.loaderVer || null, true);
      await setActiveInstance(inst.id);
      const st = await listInstancesState();
      this.instances = st.instances;
      this.activeId = st.active_id;
      showToast(`已装好「${inst.name}」，可以去启动页开玩`);
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.busy = false;
    }
  }

  private renderMarket() {
    const hint =
      this.tab === "mod"
        ? "为当前要玩的版本找模组"
        : this.tab === "modpack"
          ? "一键装好整套模组包"
          : this.tab === "resourcepack"
            ? "换皮肤材质包"
            : this.tab === "shader"
              ? "让画面更好看"
              : "给世界加玩法";
    return html`
      <p class="muted" style="margin-top:0">${hint}</p>
      <div class="search-row">
        <div class="search">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg>
          <input placeholder="搜索模组、工具、整合包…" .value=${this.query} @keydown=${(e: KeyboardEvent) => { if (e.key === "Enter") void this.onSearch(); }} @input=${(e: Event) => (this.query = (e.target as HTMLInputElement).value)} />
        </div>
        <button class="primary" type="button" ?disabled=${this.loading} @click=${() => this.onSearch()}>搜索</button>
        ${this.instances.length
          ? html`<select .value=${this.activeId ?? ""} @change=${async (e: Event) => { this.activeId = (e.target as HTMLSelectElement).value; await setActiveInstance(this.activeId); }} title="装进哪个版本">
              ${this.instances.map((i) => html`<option value=${i.id}>${i.name}</option>`)}
            </select>`
          : nothing}
      </div>
      <div class="sources" style="margin-bottom:14px">
        <span class="lbl">来源</span>
        <button class="src ${this.source === "modrinth" ? "on" : ""}" type="button" @click=${() => (this.source = "modrinth")}>
          <span class="dot mr"></span> Modrinth
        </button>
        <button class="src ${this.source === "curseforge" ? "on" : ""}" type="button" @click=${() => (this.source = "curseforge")}>
          <span class="dot cf"></span> CurseForge
        </button>
      </div>
      ${this.source === "curseforge" && !this.cf?.configured
        ? html`<div class="panel" style="margin-bottom:12px">
            <p class="muted">浏览 CurseForge 需要密钥（可在
              <a href="https://console.curseforge.com/" target="_blank" rel="noreferrer">CurseForge 控制台</a> 申请）。Modrinth 无需密钥。</p>
            <div class="bar">
              <input type="password" placeholder="粘贴密钥…" .value=${this.cfKeyDraft} @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)} />
              <button class="primary" type="button" @click=${async () => { const r = await setCurseforgeKey(this.cfKeyDraft.trim() || null); showToast(r.message); this.cf = await curseforgeConfigured(); }}>保存</button>
            </div>
          </div>`
        : nothing}
      ${this.tab === "datapack"
        ? html`<div class="bar"><label class="muted">装到哪个世界的数据包文件夹（可留空=版本级 datapacks）</label>
            <input placeholder="世界名（可选）" .value=${this.worldPick} @input=${(e: Event) => (this.worldPick = (e.target as HTMLInputElement).value)} list="worlds" />
            <datalist id="worlds">${this.worlds.map((w) => html`<option value=${w}></option>`)}</datalist>
          </div>`
        : nothing}
      ${this.error ? html`<p class="muted" style="color:var(--md-sys-color-error)">${this.error}</p>` : nothing}
      ${this.loading ? html`<p class="muted">搜索中…</p>` : nothing}
      ${this.progress && this.installingId ? html`<p class="muted">${this.progress.stage_label} · ${this.progress.detail}</p>` : nothing}
      <div class="market-grid">
        ${this.results.map((p) => this.renderCard(p))}
      </div>
      ${!this.loading && this.results.length === 0 && !this.error
        ? html`<div class="results-slot"><easymc-empty-state
            variant="panel"
            title="没有符合筛选的结果"
            description="换个关键词试试，或留空看看热门"
          ></easymc-empty-state></div>`
        : nothing}
    `;
  }

  private renderCard(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const cta =
      this.tab === "modpack" ? "安装" : this.tab === "mod" ? "装入" : "下载";
    const dl =
      typeof p.downloads === "number"
        ? p.downloads >= 1000
          ? `${(p.downloads / 1000).toFixed(p.downloads >= 10000 ? 0 : 1)}K`
          : String(p.downloads)
        : String(p.downloads ?? "");
    return html`<div class="card">
      <div class="card-top">
        ${p.icon_url ? html`<img src=${p.icon_url} alt="" />` : html`<div class="ph" aria-hidden="true"></div>`}
        <div class="body">
          <div class="name">${p.name}</div>
          <div class="desc">${p.description}</div>
          <div class="tags">
            <span class="tag">${p.source}</span>
            ${p.author ? html`<span class="tag">${p.author}</span>` : nothing}
          </div>
        </div>
      </div>
      <div class="card-foot">
        <span class="dl-stat">${dl ? `↓ ${dl}` : ""}</span>
        <div class="bar" style="margin:0">
          <button class="outline-dl" type="button" ?disabled=${!!busy} @click=${() => this.onInstall(p)}>${busy ? "…" : cta}</button>
          <button class="ghost" type="button" @click=${() => this.onExpand(p)}>${this.expandedId === p.id ? "收起" : "版本"}</button>
        </div>
      </div>
      ${this.expandedId === p.id
        ? html`<div>
            ${this.versionsLoading
              ? html`<span class="muted">加载版本…</span>`
              : this.versions.slice(0, 12).map(
                  (v) => html`<div class="bar">
                    <span class="muted">${v.version_number} · ${v.name}</span>
                    <button class="ghost" type="button" ?disabled=${!!busy} @click=${() => this.onInstall(p, v.id)}>用此版本</button>
                  </div>`,
                )}
          </div>`
        : nothing}
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-download-page": DownloadPage;
  }
}
