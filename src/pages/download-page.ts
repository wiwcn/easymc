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

  private unprog?: () => void;

  static styles = css`
    :host { display: block; }
    h1 { font-size: 1.35rem; font-weight: 650; margin: 8px 0 4px; letter-spacing: -0.02em; }
    .sub { color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem; margin-bottom: 14px; }
    .tabs { display: flex; gap: 4px; flex-wrap: wrap; margin-bottom: 14px; }
    .tabs button {
      border: none; background: transparent; cursor: pointer;
      padding: 8px 14px; border-radius: 999px;
      color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem;
    }
    .tabs button.on {
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container); font-weight: 600;
    }
    .bar {
      display: flex; gap: 8px; flex-wrap: wrap; align-items: center; margin-bottom: 12px;
    }
    input, select, button {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: 999px; padding: 8px 14px; font-size: 0.9rem;
    }
    button.primary { border: none; background: var(--md-sys-color-primary); color: var(--md-sys-color-on-primary); cursor: pointer; }
    button.ghost { background: transparent; cursor: pointer; }
    .card {
      display: flex; gap: 12px; padding: 12px; margin-bottom: 8px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      align-items: flex-start;
    }
    .card img { width: 48px; height: 48px; border-radius: 10px; object-fit: cover; background: var(--md-sys-color-surface-container); }
    .card .body { flex: 1; min-width: 0; }
    .card .name { font-weight: 600; }
    .card .desc { font-size: 0.85rem; color: var(--md-sys-color-on-surface-variant); margin-top: 2px; }
    .muted { color: var(--md-sys-color-on-surface-variant); font-size: 0.85rem; }
    .panel {
      padding: 16px; border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
    }
    .field { margin-bottom: 10px; display: flex; flex-direction: column; gap: 4px; max-width: 480px; }
    .field label { font-size: 0.85rem; color: var(--md-sys-color-on-surface-variant); }
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
    return html`
      <h1>下载</h1>
      <p class="sub">
        ${this.tab === "modpack"
          ? "一键装好整套模组包（会新建一个版本）"
          : this.active
            ? `装进当前版本：${this.active.name}`
            : "先在启动页选一个版本，或直接装整合包"}
      </p>
      <div class="tabs">
        ${tabs.map((t) => html`<button class=${this.tab === t.id ? "on" : ""} type="button" @click=${() => this.setTab(t.id)}>${t.label}</button>`)}
      </div>
      ${this.tab === "vanilla" ? this.renderVanilla() : this.renderMarket()}
    `;
  }

  private renderVanilla() {
    const filtered = this.mcVersions.filter((v) => !this.filter || v.id.includes(this.filter));
    return html`<div class="panel">
      <p class="muted">选择游戏版本，创建或装进本地版本。</p>
      <div class="field"><label>名称（可选）</label><input .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} placeholder="我的生存世界" /></div>
      <div class="bar">
        <input placeholder="筛选…" .value=${this.filter} @input=${(e: Event) => (this.filter = (e.target as HTMLInputElement).value)} />
        <label class="muted"><input type="checkbox" .checked=${this.snapshots} @change=${async (e: Event) => { this.snapshots = (e.target as HTMLInputElement).checked; await this.loadVanilla(); }} /> 含快照</label>
      </div>
      <div class="field"><label>游戏版本</label>
        <select .value=${this.pick} @change=${async (e: Event) => { this.pick = (e.target as HTMLSelectElement).value; if (this.loader !== "vanilla") { this.loaderVers = await listLoaderVersions(this.loader, this.pick); this.loaderVer = this.loaderVers[0]?.version ?? ""; } }}>
          ${filtered.slice(0, 80).map((v) => html`<option value=${v.id}>${v.id} · ${versionType(v)}</option>`)}
        </select>
      </div>
      <div class="field"><label>加载器</label>
        <select .value=${this.loader} @change=${async (e: Event) => { this.loader = (e.target as HTMLSelectElement).value as LoaderId; this.loaderVers = this.loader === "vanilla" ? [] : await listLoaderVersions(this.loader, this.pick); this.loaderVer = this.loaderVers[0]?.version ?? ""; }}>
          <option value="vanilla">原版</option><option value="fabric">Fabric</option><option value="quilt">Quilt</option><option value="forge">Forge</option><option value="neoforge">NeoForge</option>
        </select>
      </div>
      ${this.loader !== "vanilla" ? html`<div class="field"><label>加载器版本</label>
        <select .value=${this.loaderVer} @change=${(e: Event) => (this.loaderVer = (e.target as HTMLSelectElement).value)}>
          ${this.loaderVers.map((v) => html`<option value=${v.version}>${v.version}${v.stable ? "（稳定）" : ""}</option>`)}
        </select></div>` : nothing}
      <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.installVanilla()}>安装并添加版本</button>
      ${this.progress ? html`<p class="muted" style="margin-top:10px">${this.progress.stage_label} · ${this.progress.detail}</p>` : nothing}
    </div>`;
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
      <div class="bar">
        <select .value=${this.source} @change=${(e: Event) => (this.source = (e.target as HTMLSelectElement).value as ModSourceId)}>
          <option value="modrinth">Modrinth</option>
          <option value="curseforge">CurseForge</option>
        </select>
        <input style="flex:1;min-width:160px" placeholder="搜索…" .value=${this.query} @keydown=${(e: KeyboardEvent) => { if (e.key === "Enter") void this.onSearch(); }} @input=${(e: Event) => (this.query = (e.target as HTMLInputElement).value)} />
        <button class="primary" type="button" ?disabled=${this.loading} @click=${() => this.onSearch()}>搜索</button>
        ${this.instances.length
          ? html`<select .value=${this.activeId ?? ""} @change=${async (e: Event) => { this.activeId = (e.target as HTMLSelectElement).value; await setActiveInstance(this.activeId); }} title="装进哪个版本">
              ${this.instances.map((i) => html`<option value=${i.id}>${i.name}</option>`)}
            </select>`
          : nothing}
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
      ${this.results.map((p) => this.renderCard(p))}
      ${!this.loading && this.results.length === 0 && !this.error ? html`<p class="muted">输入关键词搜索，或留空看热门。</p>` : nothing}
    `;
  }

  private renderCard(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const cta =
      this.tab === "modpack" ? "安装到新版本" : this.tab === "mod" ? "装入当前版本" : "下载";
    return html`<div class="card">
      ${p.icon_url ? html`<img src=${p.icon_url} alt="" />` : html`<div style="width:48px;height:48px;border-radius:10px;background:var(--md-sys-color-surface-container)"></div>`}
      <div class="body">
        <div class="name">${p.name}</div>
        <div class="desc">${p.description}</div>
        <div class="muted" style="margin-top:4px">${p.source} · 下载 ${p.downloads}${p.author ? ` · ${p.author}` : ""}</div>
        <div class="bar" style="margin:8px 0 0">
          <button class="primary" type="button" ?disabled=${!!busy} @click=${() => this.onInstall(p)}>${busy ? "安装中…" : cta}</button>
          <button class="ghost" type="button" @click=${() => this.onExpand(p)}>${this.expandedId === p.id ? "收起版本" : "选版本"}</button>
        </div>
        ${this.expandedId === p.id
          ? html`<div style="margin-top:8px">
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
      </div>
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-download-page": DownloadPage;
  }
}
