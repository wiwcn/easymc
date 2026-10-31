import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  createInstance,
  deleteInstance,
  deleteLocalContent,
  duplicateInstance,
  exportInstance,
  importInstance,
  installLoaderVersion,
  installVersion,
  launchInstance,
  listInstalledMods,
  modDepGraph,
  fetchMissingModDeps,
  listInstancesState,
  listLoaderVersions,
  listLocalContent,
  listVersions,
  openInstanceFolder,
  removeMod,
  renameInstance,
  setActiveInstance,
  setModDisabled,
  updateInstance,
  writeLaunchScript,
  uploadLatestLog,
  containerAttach,
  containerProvision,
  containerLayerState,
  containerLayerApply,
  containerLayerDiscard,
  containerAgentWorkspace,
  containerExportEnv,
  containerImportEnv,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import { listenInstallProgress } from "../api";
import type { ContainerLayerStateDto, DepGraphReportDto, InstalledModEntryDto, InstanceDto, LoaderId, LocalEntryDto, VersionSummary } from "../types";
import { instancePlayerLabel, loaderLabel, playerVersionLabel, versionId, versionType } from "../types";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import { sortInstancesReadyFirst } from "../lib/download-ux";
import { instanceIconUrl } from "../lib/instance-icon";
import { pickEnvPackFile } from "../lib/pick-file";
import { syncIndicator } from "../lib/indicator";
import "../components/install-progress";
import "../components/empty-state";
import "../components/dep-tree";

type Tab = "settings" | "installers" | "mods" | "worlds" | "resourcepacks" | "shaders" | "export";

@customElement("easymc-instance-page")
export class InstancePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private detailId: string | null = null;
  @state() private tab: Tab = "settings";
  @state() private loading = true;
  @state() private busy = false;
  @state() private mods: InstalledModEntryDto[] = [];
  @state() private depGraph: DepGraphReportDto | null = null;
  @state() private depBusy = false;
  @state() private depMsg = "";
  @state() private layerState: ContainerLayerStateDto | null = null;
  @state() private workspaceHint = "";
  @state() private locals: LocalEntryDto[] = [];
  @state() private menuFor: string | null = null;
  @state() private nameDraft = "";
  @state() private memDraft = "";
  @state() private javaDraft = "";
  @state() private widthDraft = "";
  @state() private heightDraft = "";
  @state() private jvmDraft = "";
  @state() private catDraft = "";
  @state() private qpModeDraft: "" | "world" | "server" | "realms" = "";
  @state() private qpTargetDraft = "";
  @state() private showCreate = false;
  @state() private newName = "";
  @state() private newVersion = "1.21.1";
  @state() private newLoader: LoaderId = "vanilla";
  @state() private newLoaderVersion = "";
  @state() private loaderVersions: { version: string; stable: boolean }[] = [];
  @state() private versions: VersionSummary[] = [];
  @state() private progress: InstallProgressDto | null = null;
  @state() private alsoInstall = true;

  private unprog?: () => void;

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    :host {
      display: flex;
      flex-direction: column;
      flex: 1 0 auto;
      min-height: 100%;
      width: 100%;
      box-sizing: border-box;
    }
    /* h1 / .sub → pageChromeStyles (TYP-1). Detail pages: slightly tighter sub measure */
    .sub {
      max-width: 42em;
      margin-bottom: var(--easymc-space-3);
    }
    .toolbar { display: flex; gap: var(--easymc-space-2); flex-wrap: wrap; margin-bottom: var(--easymc-space-3); align-items: center; }
    /* CMP-1: buttons from controlStyles only; form fields keep local chrome */
    select, input {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 8px 14px;
      font-size: var(--easymc-type-body);
      font-weight: var(--easymc-weight-regular);
      font-family: inherit;
    }
    /* Compact card / toolbar CTAs — colors stay on controlStyles */
    .res-card-foot button,
    .toolbar button,
    .dep-head button {
      padding: 6px 12px;
      font-size: var(--easymc-type-label);
    }
    /* Outline danger for uninstall (ghost+danger combo); filled danger keeps controlStyles */
    button.danger.ghost {
      color: var(--md-sys-color-error);
      border-color: color-mix(in oklch, var(--md-sys-color-error) 45%, transparent);
      background: transparent;
    }
    button.danger.ghost:hover:not(:disabled) {
      background: color-mix(in oklch, var(--md-sys-color-error) 10%, transparent);
    }
    .row-item {
      display: flex; align-items: center; gap: var(--easymc-space-3);
      padding: var(--easymc-space-3) var(--easymc-space-4);
      box-sizing: border-box; /* width:100% + padding must not overflow the list */
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      border: 1px solid transparent;
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elev XOR border */
      cursor: pointer;
      text-align: left;
      width: 100%;
      min-height: 64px;
      transition: background var(--easymc-dur-press) var(--easymc-ease-out), border-color var(--easymc-dur-press) var(--easymc-ease-out), box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .row-item:hover { background: var(--md-sys-color-surface-container); }
    .row-item:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    .row-item.active {
      border-color: var(--md-sys-color-primary);
      box-shadow: none; /* CF-1: selected = outline, drop elev */
      background: color-mix(in oklch, var(--md-sys-color-primary) 6%, var(--md-sys-color-surface-container-lowest));
    }
    .row-item.not-ready {
      opacity: 0.88;
      background: var(--md-sys-color-surface-container-low);
    }
    .row-item.not-ready .icon {
      filter: grayscale(0.45);
      opacity: 0.8;
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
    }
    .icon {
      width: 40px; height: 40px; border-radius: var(--md-sys-shape-corner-medium);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      display: grid; place-items: center; font-weight: var(--easymc-weight-bold); flex-shrink: 0;
      overflow: hidden;
    }
    .icon img {
      width: 100%; height: 100%; object-fit: cover; display: block;
    }
    .meta { flex: 1; min-width: 0; }
    .meta .name {
      font-size: var(--easymc-type-body);
      font-weight: var(--easymc-weight-medium);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .meta .info {
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-regular);
      color: var(--md-sys-color-on-surface-variant);
      display: flex; flex-wrap: wrap; gap: var(--easymc-space-2); align-items: center;
    }
    .st {
      display: inline-flex; align-items: center; gap: 4px;
      font-size: var(--easymc-type-meta); font-weight: var(--easymc-weight-medium);
      padding: 2px 8px; border-radius: var(--md-sys-shape-corner-full);
    }
    .st.ok {
      background: var(--md-sys-color-success-container);
      color: var(--md-sys-color-on-success-container);
    }
    .st.pending {
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
    }
    .menu-wrap { position: relative; }
    .menu-panel {
      position: absolute; right: 0; top: 100%; z-index: 10; min-width: 180px;
      background: var(--md-sys-color-surface-container-lowest);
      border-radius: var(--md-sys-shape-corner-medium);
      box-shadow: var(--md-sys-elevation-2); padding: 6px;
      display: flex; flex-direction: column;
      /* MI-8: enter from the trigger edge; exit one step faster. */
      opacity: 0; visibility: hidden; pointer-events: none;
      transform: translateY(-6px) scale(0.96); transform-origin: top right;
      transition:
        opacity var(--easymc-dur-fast) var(--easymc-ease-out),
        transform var(--easymc-dur-fast) var(--easymc-ease-out),
        visibility 0s linear var(--easymc-dur-fast);
    }
    .menu-panel.open {
      opacity: 1; visibility: visible; pointer-events: auto; transform: none;
      transition:
        opacity var(--easymc-dur-med) var(--easymc-ease-out),
        transform var(--easymc-dur-med) var(--easymc-ease-out),
        visibility 0s;
    }
    .menu-panel button { border: none; background: transparent; text-align: left; border-radius: var(--md-sys-shape-corner-small); padding: 8px 10px; }
    .menu-panel button:hover { background: var(--md-sys-color-surface-container); }
    .menu-panel button:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    .back { margin-bottom: var(--easymc-space-2); }
    /* .tabs → controlStyles .md-tabs (MC-2, shared vocabulary + sliding indicator) */
    .panel {
      padding: var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elev XOR border — was ghost low surface */
      border: none;
    }
    .panel h2 {
      margin: 0 0 var(--easymc-space-3);
      font-size: var(--easymc-type-title);
      font-weight: var(--easymc-weight-medium);
      letter-spacing: -0.01em;
    }
    .field { display: flex; flex-direction: column; gap: var(--easymc-space-1); margin-bottom: var(--easymc-space-3); max-width: 420px; }
    .field label { font-size: var(--easymc-type-label); font-weight: var(--easymc-weight-medium); color: var(--md-sys-color-on-surface-variant); }
    .field input, .field select { border-radius: var(--md-sys-shape-corner-small); }
    .list {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 200px;
    }
    .panel.panel-empty {
      display: flex;
      flex-direction: column;
      min-height: 220px;
    }
    .panel-empty-slot {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 160px;
    }
    .list { gap: 6px; }
    .inst-skel { padding: var(--easymc-space-4) 0; }
    /* .muted → pageChrome meta; QP help uses body measure */
    .muted { max-width: 42em; line-height: 1.45; }
    .field .muted { font-size: var(--easymc-type-meta); }
    .resource-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr));
      gap: var(--easymc-grid-gap);
      margin-top: 4px;
      align-items: stretch;
    }
    @container easymc-page (max-width: 720px) {
      .resource-grid {
        grid-template-columns: repeat(
          auto-fill,
          minmax(min(var(--easymc-grid-min-dense-narrow), 100%), 1fr)
        );
      }
    }
    .res-card {
      container-type: inline-size;
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
      padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
      border-radius: var(--easymc-card-radius);
      background: var(--md-sys-color-surface);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      box-shadow: none; /* CF-1: outline inside elev panel — no nested elev */
      min-height: 128px;
      height: 100%;
      box-sizing: border-box;
    }
    .res-card.disabled { opacity: 0.62; }
    .res-card-top {
      display: flex;
      gap: var(--easymc-space-3);
      align-items: flex-start;
      flex: 0 0 auto;
    }
    .res-card img, .res-card .ph {
      width: 48px;
      height: 48px;
      border-radius: var(--md-sys-shape-corner-large);
      object-fit: cover;
      background: color-mix(in oklch, var(--md-sys-color-primary) 6%, var(--md-sys-color-surface-container));
      flex-shrink: 0;
    }
    .res-card .body { flex: 1; min-width: 0; }
    .res-card .name {
      font-weight: var(--easymc-weight-medium);
      font-size: var(--easymc-type-body);
      overflow-wrap: break-word;
      word-break: normal;
      line-height: var(--easymc-leading-body, 1.35);
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
    }
    .res-card .meta {
      margin-top: 2px;
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-regular);
      color: var(--md-sys-color-on-surface-variant);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .res-card-foot {
      display: flex;
      flex-wrap: nowrap;
      gap: var(--easymc-space-2);
      margin-top: auto;
      padding-top: var(--easymc-space-1);
      align-items: center;
      justify-content: flex-end;
    }
    .res-card-foot button { white-space: nowrap; }
    @container (max-width: 180px) {
      .res-card-foot {
        flex-wrap: wrap;
        justify-content: stretch;
      }
      .res-card-foot button { flex: 1 1 auto; }
    }
    .local-row {
      display: flex; justify-content: space-between; align-items: center;
      padding: 8px 0; border-bottom: 1px solid var(--md-sys-color-outline-variant);
    }
    .dep-panel {
      margin-top: var(--easymc-space-4);
      padding-top: var(--easymc-space-4);
      border-top: 1px solid var(--md-sys-color-outline-variant);
    }
    .dep-head {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
      align-items: center;
      margin-bottom: var(--easymc-space-3);
    }
    .dep-head h3 {
      margin: 0;
      flex: 1;
      font-size: var(--easymc-type-title);
      font-weight: var(--easymc-weight-medium);
      letter-spacing: -0.01em;
    }
    .dep-summary {
      margin: 0 0 var(--easymc-space-2);
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-regular);
      line-height: var(--easymc-leading-body, 1.45);
    }
    .dep-alerts {
      margin: 0 0 var(--easymc-space-3);
      padding: var(--easymc-space-2) var(--easymc-space-3);
      border-radius: var(--md-sys-shape-corner-medium);
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
    }
    .dep-alerts li { margin: 2px 0; }
    .dep-tree-wrap {
      max-height: 360px;
      overflow: auto;
      padding: var(--easymc-space-2) var(--easymc-space-3) var(--easymc-space-3);
      border-radius: var(--md-sys-shape-corner-medium);
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      box-shadow: none; /* CF-1: outline only inside elev panel */
    }
    @media (prefers-reduced-motion: reduce) {
      .row-item { transition: none; }
      .menu-panel, .menu-panel.open { transition: none; transform: none; }
    }
  `,
  ];

  connectedCallback() {
    super.connectedCallback();
    void this.load();
    void listenInstallProgress((p) => (this.progress = p)).then((u) => (this.unprog = u));
    window.addEventListener("click", () => { if (this.menuFor) this.menuFor = null; });
  }
  disconnectedCallback() {
    this.unprog?.();
    super.disconnectedCallback();
  }

  private revealed = false;

  protected updated() {
    syncIndicator(this.renderRoot);
    // C-5: fade content in once the first load resolves (skeleton → content).
    if (!this.loading && !this.revealed) {
      this.revealed = true;
      this.classList.add("md-loaded");
    }
  }

  private get detail(): InstanceDto | null {
    return this.instances.find((i) => i.id === this.detailId) ?? null;
  }

  private async load() {
    this.loading = true;
    try {
      const st = await listInstancesState();
      this.instances = st.instances;
      this.activeId = st.active_id;
    } finally {
      this.loading = false;
    }
  }

  private async openDetail(id: string) {
    this.detailId = id;
    this.tab = "settings";
    const d = this.instances.find((i) => i.id === id);
    if (!d) return;
    this.nameDraft = d.name;
    this.memDraft = d.memory_mb ? String(d.memory_mb) : "";
    this.javaDraft = d.java_path ?? "";
    this.widthDraft = d.width ? String(d.width) : "";
    this.heightDraft = d.height ? String(d.height) : "";
    this.jvmDraft = d.jvm_args ?? "";
    this.catDraft = d.category ?? "";
    const qp = d.quick_play;
    if (qp?.type === "world") {
      this.qpModeDraft = "world";
      this.qpTargetDraft = qp.name || "";
    } else if (qp?.type === "server") {
      this.qpModeDraft = "server";
      this.qpTargetDraft = qp.address || "";
    } else if (qp?.type === "realms") {
      this.qpModeDraft = "realms";
      this.qpTargetDraft = qp.id || "";
    } else {
      this.qpModeDraft = "";
      this.qpTargetDraft = "";
    }
    await this.refreshLayer(d.id);
    await this.refreshTab();
  }

  private async refreshTab() {
    const d = this.detail;
    if (!d) return;
    if (this.tab === "settings") await this.refreshLayer(d.id);
    if (this.tab === "mods") {
      this.mods = await listInstalledMods(d.id);
      try {
        this.depGraph = await modDepGraph(d.id);
        this.depMsg = this.depGraph.summary_zh;
      } catch (e) {
        this.depGraph = null;
        this.depMsg = friendlyError(e);
      }
    }
    if (this.tab === "worlds") this.locals = await listLocalContent(d.id, "saves");
    if (this.tab === "resourcepacks") this.locals = await listLocalContent(d.id, "resourcepacks");
    if (this.tab === "shaders") this.locals = await listLocalContent(d.id, "shaderpacks");
  }

  private async setTab(t: Tab) {
    this.tab = t;
    await this.refreshTab();
  }

  private async fetchMissingDeps(instanceId: string) {
    if (this.depBusy) return;
    this.depBusy = true;
    this.depMsg = "正在查找并下载还缺的模组…";
    try {
      const r = await fetchMissingModDeps(instanceId);
      this.depMsg = r.message_zh;
      showToast(r.message_zh);
      this.mods = await listInstalledMods(instanceId);
      this.depGraph = await modDepGraph(instanceId);
    } catch (e) {
      this.depMsg = friendlyError(e);
      showToast(this.depMsg);
    } finally {
      this.depBusy = false;
    }
  }

  private goDownload(tab?: string) {
    this.dispatchEvent(
      new CustomEvent("nav-change", {
        detail: "download",
        bubbles: true,
        composed: true,
      }),
    );
    if (tab) {
      sessionStorage.setItem("easymc-download-tab", tab);
    }
  }

  private async refreshLayer(id: string) {
    try {
      this.layerState = await containerLayerState(id);
      const ws = await containerAgentWorkspace(id);
      this.workspaceHint = ws.message_zh;
    } catch {
      this.layerState = null;
      this.workspaceHint = "";
    }
  }

  private async act(fn: () => Promise<void>) {
    this.busy = true;
    try {
      await fn();
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.busy = false;
      this.menuFor = null;
    }
  }

  render() {
    if (this.loading) {
      return html`<div class="inst-skel" aria-busy="true" aria-label="正在加载">
        <div class="skel skel-line w40"></div>
        <div class="skel skel-block" style="height:120px;margin-top:12px;border-radius:var(--md-sys-shape-corner-large)"></div>
        <div class="skel skel-block" style="height:80px;margin-top:10px;border-radius:var(--md-sys-shape-corner-large)"></div>
      </div>`;
    }
    if (this.detailId && this.detail) return this.renderDetail(this.detail);
    return html`
      <h1>版本管理</h1>
      <p class="sub">管理本地游戏版本：设置、模组、世界与导出。</p>
      <div class="toolbar">
        <button class="primary" type="button" @click=${() => this.openCreate()}>添加版本</button>
        <button class="ghost" type="button" @click=${() => this.goDownload()}>去下载</button>
        <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.onImport()}>导入好友分享</button>
      </div>
      <div class="list">
        ${this.instances.length === 0
          ? html`<easymc-empty-state
              variant="panel"
              title="咦？似乎什么都没有…"
              description="先添加一个版本，或去「下载」装整合包。"
            >
              <button class="primary" type="button" @click=${() => this.openCreate()}>添加版本</button>
              <button class="ghost" type="button" @click=${() => this.goDownload("modpack")}>去下载整合包</button>
            </easymc-empty-state>`
          : sortInstancesReadyFirst(this.instances).map((i) => this.renderRow(i))}
      </div>
      ${this.showCreate ? this.renderCreate() : nothing}
    `;
  }

  private renderRow(i: InstanceDto) {
    const ver = versionId(i.version);
    const label = instancePlayerLabel(i);
    const custom = (i.name || "").trim();
    const showCustom = custom && custom !== label && !custom.startsWith(ver);
    const ready = !!i.version_ready;
    return html`
      <div class="row-item ${i.id === this.activeId ? "active" : ""} ${ready ? "" : "not-ready"}" @click=${() => this.openDetail(i.id)}>
        <div class="icon" aria-hidden="true"><img src=${instanceIconUrl(i)} alt="" width="40" height="40" loading="lazy" /></div>
        <div class="meta">
          <div class="name">${label}</div>
          <div class="info">
            <span class="st ${ready ? "ok" : "pending"}">${ready ? "已就绪" : "未安装"}</span>
            ${showCustom ? html`<span>${custom}</span>` : nothing}
            <span>${i.category ?? "未分类"}</span>
          </div>
        </div>
        <div class="menu-wrap" @click=${(e: Event) => e.stopPropagation()}>
          <button type="button" class="ghost" title="操作" @click=${() => (this.menuFor = this.menuFor === i.id ? null : i.id)}>⋮</button>
          ${this.renderMenu(i, this.menuFor === i.id)}
        </div>
      </div>
    `;
  }

  private renderMenu(i: InstanceDto, open: boolean) {
    return html`<div class="menu-panel ${open ? "open" : ""}" role="menu" aria-hidden=${open ? "false" : "true"}>
      <button type="button" @click=${() => this.act(async () => { await setActiveInstance(i.id); await this.load(); showToast("已设为启动页默认版本"); })}>设为启动默认</button>
      <button type="button" @click=${() => this.act(async () => { await launchInstance(i.id); showToast("游戏已启动"); })}>启动游戏</button>
      <button type="button" @click=${() => this.act(async () => {
        const n = prompt("新名称", i.name);
        if (!n?.trim()) return;
        await renameInstance(i.id, n.trim());
        await this.load();
        showToast("已改名");
      })}>改名</button>
      <button type="button" @click=${() => this.act(async () => { await duplicateInstance(i.id); await this.load(); showToast("已复制"); })}>复制版本</button>
      <button type="button" @click=${() => this.act(async () => { showToast(await openInstanceFolder(i.id, "game")); })}>打开游戏文件夹</button>
      <button type="button" @click=${() => this.act(async () => { const r = await writeLaunchScript(i.id); showToast(r.message_zh); })}>生成启动脚本</button>
      <button type="button" @click=${() => this.act(async () => { const r = await exportInstance(i.id, false); showToast(r.message_zh); })}>导出（轻量）</button>
      <button type="button" class="danger" @click=${() => this.act(async () => {
        if (!confirm(`删除「${i.name}」？游戏文件也会删掉。`)) return;
        await deleteInstance(i.id, true);
        if (this.detailId === i.id) this.detailId = null;
        await this.load();
        showToast("已删除");
      })}>删除</button>
    </div>`;
  }

  private renderDetail(d: InstanceDto) {
    const tabs: { id: Tab; label: string }[] = [
      { id: "settings", label: "设置" },
      { id: "installers", label: "自动安装" },
      { id: "mods", label: "模组" },
      { id: "worlds", label: "世界" },
      { id: "resourcepacks", label: "资源包" },
      { id: "shaders", label: "光影" },
      { id: "export", label: "导出" },
    ];
    return html`
      <button class="ghost back" type="button" @click=${() => (this.detailId = null)}>← 返回列表</button>
      <h1>${instancePlayerLabel(d)}</h1>
      <p class="sub">${(d.name || "").trim() && (d.name || "").trim() !== instancePlayerLabel(d) ? `${d.name} · ` : ""}${d.loader_version ? `加载器 ${d.loader_version}` : "模组、世界与导出"}</p>
      <div class="md-tabs" role="tablist" aria-label="版本管理">
        <span class="md-tab-ind" aria-hidden="true"></span>
        ${tabs.map((t) => html`<button class=${this.tab === t.id ? "on" : ""} type="button" role="tab" aria-selected=${this.tab === t.id} @click=${() => this.setTab(t.id)}>${t.label}</button>`)}
      </div>
      <easymc-install-progress .progress=${this.progress}></easymc-install-progress>
      ${this.renderTab(d)}
    `;
  }

  private renderTab(d: InstanceDto) {
    switch (this.tab) {
      case "settings":
        return html`<div class="panel">
          <h2>游戏设置</h2>
          <div class="field"><label>自定义名称（可选）</label><input .value=${this.nameDraft} @input=${(e: Event) => (this.nameDraft = (e.target as HTMLInputElement).value)} /></div>
          <div class="field"><label>分类</label><input placeholder="生存 / 创造 / 模组" .value=${this.catDraft} @input=${(e: Event) => (this.catDraft = (e.target as HTMLInputElement).value)} /></div>
          <div class="field">
            <label>游戏内存 ${this.memDraft.trim() ? "" : "（当前用全局默认）"}</label>
            <div class="slider-row">
              <input
                class="md-slider"
                type="range"
                min="512"
                max="16384"
                step="256"
                .value=${this.memDraft.trim() || "2048"}
                aria-label="本版本游戏内存（兆字节）"
                aria-valuetext=${this.memDraft.trim() ? `${this.memDraft} 兆字节` : "使用全局默认"}
                @input=${(e: Event) => {
                  this.memDraft = (e.target as HTMLInputElement).value;
                }}
              />
              <span class="slider-value" aria-live="polite"
                >${this.memDraft.trim()
                  ? html`${this.memDraft}<span class="unit">MB</span>`
                  : html`<span class="unit">全局</span>`}</span
              >
            </div>
            <div class="row" style="margin-top:8px">
              <button
                class="text"
                type="button"
                ?disabled=${!this.memDraft.trim()}
                @click=${() => (this.memDraft = "")}
              >
                用全局默认
              </button>
            </div>
          </div>
          <div class="field"><label>Java 位置（可选）</label><input placeholder="一般不用填，留空会自动选" .value=${this.javaDraft} @input=${(e: Event) => (this.javaDraft = (e.target as HTMLInputElement).value)} /></div>
          <div class="field"><label>窗口宽 × 高（可选）</label>
            <div style="display:flex;gap:8px">
              <input type="number" placeholder="宽" .value=${this.widthDraft} @input=${(e: Event) => (this.widthDraft = (e.target as HTMLInputElement).value)} />
              <input type="number" placeholder="高" .value=${this.heightDraft} @input=${(e: Event) => (this.heightDraft = (e.target as HTMLInputElement).value)} />
            </div>
          </div>
          <div class="field"><label>额外启动参数（高级）</label><input placeholder="不懂就留空" .value=${this.jvmDraft} @input=${(e: Event) => (this.jvmDraft = (e.target as HTMLInputElement).value)} /></div>
          <div class="field" style="margin-top:12px;padding-top:12px;border-top:1px solid var(--md-sys-color-outline-variant)">
            <label>快速进入</label>
            <p class="muted" style="margin:4px 0 8px">启动后跳过标题屏，直接进世界或服务器（需 Minecraft 1.20+；更早的版本只能直连服务器）。</p>
            <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:8px">
              <select
                aria-label="快速进入方式"
                .value=${this.qpModeDraft}
                @change=${(e: Event) => {
                  this.qpModeDraft = (e.target as HTMLSelectElement).value as typeof this.qpModeDraft;
                }}
              >
                <option value="">不使用</option>
                <option value="world">单人世界</option>
                <option value="server">多人服务器</option>
                <option value="realms">Realms</option>
              </select>
              <input
                style="flex:1;min-width:160px"
                placeholder=${this.qpModeDraft === "server"
                  ? "地址，如 play.example.com:25565"
                  : this.qpModeDraft === "realms"
                    ? "Realms 世界 ID"
                    : "存档文件夹名（saves 下）"}
                .value=${this.qpTargetDraft}
                ?disabled=${!this.qpModeDraft}
                @input=${(e: Event) => (this.qpTargetDraft = (e.target as HTMLInputElement).value)}
              />
            </div>
            ${this.qpModeDraft
              ? html`<p class="muted" style="margin:0">当前：${this.qpModeDraft === "world" ? "世界" : this.qpModeDraft === "server" ? "服务器" : "Realms"} → ${this.qpTargetDraft.trim() || "（还没填目标）"}</p>`
              : nothing}
          </div>
          <div class="row" style="display:flex;gap:8px;flex-wrap:wrap;margin:12px 0">
            <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.saveSettings(d)}>保存</button>
            <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
              const r = await uploadLatestLog(d.id);
              showToast(r.message_zh);
              try { await navigator.clipboard.writeText(r.url); showToast("链接已复制"); } catch { /* ignore */ }
            })}>上传游戏日志（排查用）</button>
          </div>
          ${d.container
            ? html`<div class="field" style="margin-top:20px;padding-top:16px;border-top:1px solid var(--md-sys-color-outline-variant)">
                <h2 style="margin:0 0 8px;font-size:var(--easymc-type-title)">隔离改动</h2>
                <p class="muted" style="margin:0 0 10px">助手改模组或配置后，可在这里「应用」保留，或「丢弃」恢复原样（存档不会动）。</p>
                <p class="muted" style="margin:0 0 10px">${this.layerState?.message_zh || this.workspaceHint || "加载层状态…"}</p>
                <div class="row" style="display:flex;gap:8px;flex-wrap:wrap">
                  <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                    const r = await containerProvision(d.id);
                    showToast(r.message_zh);
                    await this.refreshLayer(d.id);
                    await this.load();
                  })}>准备隔离环境</button>
                  <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                    const r = await containerLayerApply(d.id);
                    showToast(r.message_zh);
                    this.layerState = r.state;
                  })}>应用更改</button>
                  <button class="danger ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                    if (!confirm("丢弃未应用的模组/配置等更改？（存档不会动）")) return;
                    const r = await containerLayerDiscard(d.id);
                    showToast(r.message_zh);
                    this.layerState = r.state;
                  })}>丢弃更改</button>
                  <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                    const r = await containerExportEnv(d.id, { pack: false });
                    showToast(r.message_zh);
                  })}>分享配置说明</button>
                  <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                    const r = await containerExportEnv(d.id, { pack: true });
                    showToast(r.message_zh);
                  })}>分享模组与配置</button>
                </div>
              </div>`
            : html`<div class="field" style="margin-top:20px;padding-top:16px;border-top:1px solid var(--md-sys-color-outline-variant)">
                <h2 style="margin:0 0 8px;font-size:var(--easymc-type-title)">版本隔离</h2>
                <p class="muted" style="margin:0 0 10px">可选。开启后，这个版本的模组与配置单独存放，不影响其它版本。请先在设置「启动」里开好隔离环境。</p>
                <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                  await containerAttach(d.id);
                  showToast("已为这个版本开启隔离");
                  await this.load();
                  await this.refreshLayer(d.id);
                })}>开启版本隔离</button>
              </div>`}
        </div>`;
      case "installers":
        return html`<div class="panel">
          <h2>自动安装 / 加载器</h2>
          <p class="muted">给这个版本安装游戏文件或更换加载器。装好后回到启动页即可开玩。</p>
          <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:8px">
            <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
              const ver = versionId(d.version);
              if (d.loader === "vanilla") await installVersion(ver, true);
              else await installLoaderVersion(ver, d.loader, d.loader_version ?? null, true);
              await this.load();
              showToast("安装完成");
            })}>${d.version_ready ? "重新安装游戏文件" : "安装游戏文件"}</button>
            <button class="ghost" type="button" @click=${() => this.goDownload("vanilla")}>去下载·原版选其他版本</button>
          </div>
        </div>`;
      case "mods":
        return html`<div class="panel ${this.mods.length === 0 ? "panel-empty" : ""}">
          <h2>已安装的模组</h2>
          ${this.mods.length === 0
            ? html`<div class="panel-empty-slot"><easymc-empty-state
                variant="panel"
                title="还没有模组"
                description="去「下载」挑几个装上吧"
              >
                <button class="primary" type="button" @click=${() => this.goDownload("mod")}>去下载模组</button>
              </easymc-empty-state></div>`
            : html`
                <div class="resource-grid">
                  ${this.mods.map((m) => {
                    const disabled = m.disabled;
                    const label = m.filename.replace(/\.jar(\.disabled)?$/i, "");
                    const kb = Math.max(1, Math.round(m.size / 1024));
                    return html`<div class="res-card ${disabled ? "disabled" : ""}">
                      <div class="res-card-top">
                        ${m.icon_url
                          ? html`<img src=${m.icon_url} alt="" loading="lazy" />`
                          : html`<div class="ph" aria-hidden="true"></div>`}
                        <div class="body">
                          <div class="name" title=${m.filename}>${label}</div>
                          <div class="meta">${kb} KB${disabled ? " · 已禁用" : ""}${m.source ? ` · ${m.source}` : ""}</div>
                        </div>
                      </div>
                      <div class="res-card-foot">
                        <button class="danger ghost" type="button" @click=${() => this.act(async () => { await removeMod(m.filename, d.id); await this.refreshTab(); showToast("已卸载"); })}>卸载</button>
                        <button class="ghost" type="button" @click=${() => this.act(async () => { await setModDisabled(m.filename, !disabled, d.id); await this.refreshTab(); })}>${disabled ? "启用" : "禁用"}</button>
                      </div>
                    </div>`;
                  })}
                </div>
                <div class="dep-panel">
                  <div class="dep-head">
                    <h3>模组互相需要什么</h3>
                    <button class="ghost" type="button" ?disabled=${this.depBusy} @click=${() => this.refreshTab()}>重新检查</button>
                    <button
                      class="primary"
                      type="button"
                      ?disabled=${this.depBusy || !(this.depGraph?.missing_ids.length)}
                      @click=${() => this.fetchMissingDeps(d.id)}
                    >${this.depBusy ? "下载中…" : "一键补齐还缺的模组"}</button>
                  </div>
                  <p class="dep-summary">${this.depMsg || (this.depGraph?.summary_zh ?? "正在查看模组互相需要什么…")}</p>
                  ${this.depGraph?.conflict_messages?.length
                    ? html`<ul class="dep-alerts">${this.depGraph.conflict_messages.map((m) => html`<li>${m}</li>`)}</ul>`
                    : nothing}
                  ${this.depGraph
                    ? html`<div class="dep-tree-wrap"><easymc-dep-tree .roots=${this.depGraph.roots} compact></easymc-dep-tree></div>`
                    : nothing}
                </div>
              `}
        </div>`;
      case "worlds":
        return this.renderLocals(d, "saves", "世界 / 存档", "还没有存档 — 进游戏创建世界后会出现在这里");
      case "resourcepacks":
        return this.renderLocals(d, "resourcepacks", "资源包", "还没有资源包", "resourcepack");
      case "shaders":
        return this.renderLocals(d, "shaderpacks", "光影", "还没有光影", "shader");
      case "export":
        return html`<div class="panel">
          <h2>发给好友</h2>
          <p class="muted">导出这个版本的配置，或导入好友发来的分享包。</p>
          <div style="display:flex;gap:8px;flex-wrap:wrap">
            <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => { showToast((await exportInstance(d.id, false)).message_zh); })}>导出轻量包</button>
            <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => { showToast((await exportInstance(d.id, true)).message_zh); })}>导出这个版本（含模组）</button>
            <button class="ghost" type="button" @click=${() => this.onImport()}>导入…</button>
          </div>
          <h2 style="margin-top:var(--easymc-space-5)">分享给好友</h2>
          <p class="muted">把模组、配置和加速偏好打包发给好友（不含存档）。对方导入后会得到一个新版本。</p>
          <div style="display:flex;gap:8px;flex-wrap:wrap">
            <button class="tonal" type="button" ?disabled=${this.busy || !d.container} @click=${() => this.act(async () => {
              showToast((await containerExportEnv(d.id, { pack: false })).message_zh);
            })}>分享配置说明（轻量）</button>
            <button class="ghost" type="button" ?disabled=${this.busy || !d.container} @click=${() => this.act(async () => {
              showToast((await containerExportEnv(d.id, { pack: true })).message_zh);
            })}>分享模组与配置（完整）</button>
            <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.onImportEnv()}>导入好友分享…</button>
          </div>
          ${!d.container ? html`<p class="muted" style="margin-top:8px">这个版本还没开「版本隔离」。可先在下方开启，或直接导入好友的分享包新建一个。</p>` : nothing}
        </div>`;
    }
  }

  private renderLocals(d: InstanceDto, kind: string, title: string, empty: string, downloadTab?: string) {
    const isEmpty = this.locals.length === 0;
    return html`<div class="panel ${isEmpty ? "panel-empty" : ""}">
      <h2>${title}</h2>
      <div style="display:flex;gap:var(--easymc-space-2);margin-bottom:var(--easymc-space-3);flex-wrap:wrap">
        <button class="ghost" type="button" @click=${() => this.act(async () => { showToast(await openInstanceFolder(d.id, kind === "saves" ? "saves" : kind)); })}>打开文件夹</button>
        ${downloadTab ? html`<button class="primary" type="button" @click=${() => this.goDownload(downloadTab)}>去下载</button>` : nothing}
      </div>
      ${isEmpty
        ? html`<div class="panel-empty-slot"><easymc-empty-state
            variant="panel"
            title=${empty}
            description=""
          ></easymc-empty-state></div>`
        : html`<div class="resource-grid">
          ${this.locals.map((e) => html`<div class="res-card">
            <div class="res-card-top">
              ${e.icon_url
                ? html`<img src=${e.icon_url} alt="" loading="lazy" />`
                : html`<div class="ph" aria-hidden="true"></div>`}
              <div class="body">
                <div class="name" title=${e.name}>${e.name}${e.is_dir ? "/" : ""}</div>
                <div class="meta">${Math.max(1, Math.round(e.size / 1024))} KB</div>
              </div>
            </div>
            <div class="res-card-foot">
              ${kind === "saves" && e.is_dir
                ? html`<button class="ghost" type="button" @click=${() => this.act(async () => {
                    await updateInstance(d.id, { quick_play_mode: "world", quick_play_target: e.name });
                    this.qpModeDraft = "world";
                    this.qpTargetDraft = e.name;
                    await this.load();
                    showToast(`已设为快速进入「${e.name}」`);
                  })}>设为快速进入</button>
                  <button class="tonal" type="button" @click=${() => this.act(async () => {
                    await updateInstance(d.id, { quick_play_mode: "world", quick_play_target: e.name });
                    await launchInstance(d.id);
                    showToast(`正在直进「${e.name}」…`);
                    await this.load();
                  })}>直进</button>`
                : nothing}
              <button class="danger ghost" type="button" @click=${() => this.act(async () => {
                if (!confirm(`删除 ${e.name}？`)) return;
                await deleteLocalContent(d.id, kind, e.name);
                await this.refreshTab();
              })}>删除</button>
            </div>
          </div>`)}
        </div>`}
    </div>`;
  }

  private async saveSettings(d: InstanceDto) {
    await this.act(async () => {
      const patch: Record<string, unknown> = {
        name: this.nameDraft.trim() || d.name,
        category: this.catDraft.trim(),
      };
      if (this.memDraft.trim()) patch.memory_mb = Number(this.memDraft);
      else patch.clear_memory = true;
      if (this.javaDraft.trim()) patch.java_path = this.javaDraft.trim();
      else patch.clear_java = true;
      if (this.widthDraft && this.heightDraft) {
        patch.width = Number(this.widthDraft);
        patch.height = Number(this.heightDraft);
      } else patch.clear_resolution = true;
      if (this.jvmDraft.trim()) patch.jvm_args = this.jvmDraft.trim();
      else patch.clear_jvm_args = true;
      if (this.qpModeDraft && this.qpTargetDraft.trim()) {
        patch.quick_play_mode = this.qpModeDraft;
        patch.quick_play_target = this.qpTargetDraft.trim();
      } else {
        patch.clear_quick_play = true;
      }
      await updateInstance(d.id, patch);
      await this.load();
      showToast("已保存");
    });
  }

  private async openCreate() {
    this.showCreate = true;
    this.versions = await listVersions(false);
  }

  private async onImportEnv() {
    await this.act(async () => {
      const path = await pickEnvPackFile();
      if (!path) return;
      const r = await containerImportEnv(path);
      showToast(r.message_zh);
      await this.load();
      await this.openDetail(r.instance.id);
      this.tab = "export";
    });
  }

  private async onImport() {
    const path = prompt("分享包完整路径");
    if (!path?.trim()) return;
    await this.act(async () => {
      const r = await importInstance(path.trim());
      showToast(r.message_zh);
      await this.load();
    });
  }

  private renderCreate() {
    return html`<div class="panel" style="margin-top:16px">
      <h2>添加版本</h2>
      <div class="field"><label>名称</label><input .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} /></div>
      <div class="field"><label>游戏版本</label>
        <select .value=${this.newVersion} @change=${async (e: Event) => { this.newVersion = (e.target as HTMLSelectElement).value; if (this.newLoader !== "vanilla") this.loaderVersions = await listLoaderVersions(this.newLoader, this.newVersion); }}>
          ${this.versions.slice(0, 60).map((v) => html`<option value=${v.id}>${v.id} · ${versionType(v)}</option>`)}
        </select>
      </div>
      <div class="field"><label>加载器</label>
        <select .value=${this.newLoader} @change=${async (e: Event) => { this.newLoader = (e.target as HTMLSelectElement).value as LoaderId; this.loaderVersions = this.newLoader === "vanilla" ? [] : await listLoaderVersions(this.newLoader, this.newVersion); this.newLoaderVersion = this.loaderVersions[0]?.version ?? ""; }}>
          <option value="vanilla">原版</option><option value="fabric">Fabric</option><option value="quilt">Quilt</option><option value="forge">Forge</option><option value="neoforge">NeoForge</option>
        </select>
      </div>
      ${this.newLoader !== "vanilla" ? html`<div class="field"><label>加载器版本</label>
        <select .value=${this.newLoaderVersion} @change=${(e: Event) => (this.newLoaderVersion = (e.target as HTMLSelectElement).value)}>
          ${this.loaderVersions.map((v) => html`<option value=${v.version}>${v.version}</option>`)}
        </select></div>` : nothing}
      <label class="muted"><input type="checkbox" .checked=${this.alsoInstall} @change=${(e: Event) => (this.alsoInstall = (e.target as HTMLInputElement).checked)} /> 添加后立刻安装</label>
      <div style="margin-top:10px;display:flex;gap:8px">
        <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
          const inst = await createInstance(this.newName.trim() || playerVersionLabel(this.newVersion, this.newLoader), this.newVersion, this.newLoader, this.newLoader === "vanilla" ? null : this.newLoaderVersion || null);
          if (this.alsoInstall) {
            if (this.newLoader === "vanilla") await installVersion(this.newVersion, true);
            else await installLoaderVersion(this.newVersion, this.newLoader, this.newLoaderVersion || null, true);
          }
          this.showCreate = false;
          await this.load();
          await this.openDetail(inst.id);
          showToast(`已添加「${inst.name}」`);
        })}>添加</button>
        <button class="ghost" type="button" @click=${() => (this.showCreate = false)}>取消</button>
      </div>
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-instance-page": InstancePage;
  }
}
