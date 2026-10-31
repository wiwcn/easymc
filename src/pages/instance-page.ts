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
import type { ContainerLayerStateDto, InstanceDto, LoaderId, LocalEntryDto, VersionSummary } from "../types";
import { instancePlayerLabel, loaderLabel, playerVersionLabel, versionId, versionType } from "../types";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import { pickEnvPackFile } from "../lib/pick-file";
import { syncIndicator } from "../lib/indicator";
import "../components/install-progress";
import "../components/empty-state";

type Tab = "settings" | "installers" | "mods" | "worlds" | "resourcepacks" | "shaders" | "export";

@customElement("easymc-instance-page")
export class InstancePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private detailId: string | null = null;
  @state() private tab: Tab = "settings";
  @state() private loading = true;
  @state() private busy = false;
  @state() private mods: string[] = [];
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
    h1 { font-size: var(--easymc-type-display); font-weight: var(--easymc-weight-medium); margin: 0 0 2px; letter-spacing: -0.02em; }
    .sub { color: var(--md-sys-color-on-surface-variant); font-size: var(--easymc-type-label); margin: 0 0 12px; }
    .toolbar { display: flex; gap: 8px; flex-wrap: wrap; margin-bottom: 12px; }
    button, select, input {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 8px 14px;
      font-size: var(--easymc-type-body);
      cursor: pointer;
    }
    /* primary / ghost → controlStyles (CMP-1) */
    button.danger { color: var(--md-sys-color-error); }
    .row-item {
      display: flex; align-items: center; gap: 12px;
      padding: 12px 14px;
      box-sizing: border-box; /* width:100% + padding must not overflow the list */
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid transparent;
      cursor: pointer;
      text-align: left;
      width: 100%;
      transition: background var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .row-item:hover { background: var(--md-sys-color-surface-container); }
    .row-item.active { border-color: var(--md-sys-color-primary); }
    .icon {
      width: 40px; height: 40px; border-radius: var(--md-sys-shape-corner-medium);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      display: grid; place-items: center; font-weight: 700; flex-shrink: 0;
    }
    .meta { flex: 1; min-width: 0; }
    .meta .name { font-weight: 600; }
    .meta .info { font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant); }
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
    .back { margin-bottom: 8px; }
    /* .tabs → controlStyles .md-tabs (MC-2, shared vocabulary + sliding indicator) */
    .panel {
      padding: 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
    }
    .panel h2 { margin: 0 0 10px; font-size: var(--easymc-type-title); }
    .field { display: flex; flex-direction: column; gap: 4px; margin-bottom: 12px; max-width: 420px; }
    .field label { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); }
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
    .muted { color: var(--md-sys-color-on-surface-variant); font-size: var(--easymc-type-label); }
    .local-row {
      display: flex; justify-content: space-between; align-items: center;
      padding: 8px 0; border-bottom: 1px solid var(--md-sys-color-outline-variant);
    }
    .mod-row { display: flex; gap: 8px; align-items: center; padding: 6px 0; }
    .mod-row code { flex: 1; font-size: var(--easymc-type-meta); word-break: break-all; }
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
    await this.refreshLayer(d.id);
    await this.refreshTab();
  }

  private async refreshTab() {
    const d = this.detail;
    if (!d) return;
    if (this.tab === "settings") await this.refreshLayer(d.id);
    if (this.tab === "mods") this.mods = await listInstalledMods(d.id);
    if (this.tab === "worlds") this.locals = await listLocalContent(d.id, "saves");
    if (this.tab === "resourcepacks") this.locals = await listLocalContent(d.id, "resourcepacks");
    if (this.tab === "shaders") this.locals = await listLocalContent(d.id, "shaderpacks");
  }

  private async setTab(t: Tab) {
    this.tab = t;
    await this.refreshTab();
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
        <button class="ghost" type="button" @click=${() => this.goDownload()}>去下载中心</button>
        <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.onImport()}>导入好友分享</button>
      </div>
      <div class="list">
        ${this.instances.length === 0
          ? html`<easymc-empty-state
              variant="panel"
              title="咦？似乎什么都没有…"
              description="点「添加版本」，或去下载整合包逛逛"
            >
              <button class="ghost" type="button" @click=${() => this.goDownload("modpack")}>去下载整合包</button>
            </easymc-empty-state>`
          : this.instances.map((i) => this.renderRow(i))}
      </div>
      ${this.showCreate ? this.renderCreate() : nothing}
    `;
  }

  private renderRow(i: InstanceDto) {
    const ver = versionId(i.version);
    const label = instancePlayerLabel(i);
    const custom = (i.name || "").trim();
    const showCustom = custom && custom !== label && !custom.startsWith(ver);
    return html`
      <div class="row-item ${i.id === this.activeId ? "active" : ""}" @click=${() => this.openDetail(i.id)}>
        <div class="icon" aria-hidden="true">${ver.slice(0, 1) || "?"}</div>
        <div class="meta">
          <div class="name">${label}</div>
          <div class="info">${showCustom ? `${custom} · ` : ""}${i.version_ready ? "已安装" : "未安装"} · ${i.category ?? "未分类"}</div>
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
      <p class="sub">${(d.name || "").trim() && (d.name || "").trim() !== instancePlayerLabel(d) ? `${d.name} · ` : ""}${d.loader_version ? `加载器 ${d.loader_version} · ` : ""}管理模组、世界与导出</p>
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
          <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.saveSettings(d)}>保存</button>
          ${d.container
            ? html`<div class="field" style="margin-top:20px;padding-top:16px;border-top:1px solid var(--md-sys-color-outline-variant)">
                <h2 style="margin:0 0 8px;font-size:var(--easymc-type-title)">隔离层改动</h2>
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
                  })}>导出环境包</button>
                  <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                    const r = await containerExportEnv(d.id, { pack: true });
                    showToast(r.message_zh);
                  })}>导出完整环境包</button>
                </div>
              </div>`
            : html`<div class="field" style="margin-top:20px;padding-top:16px;border-top:1px solid var(--md-sys-color-outline-variant)">
                <h2 style="margin:0 0 8px;font-size:var(--easymc-type-title)">容器隔离</h2>
                <p class="muted" style="margin:0 0 10px">可选。开启后，这个版本的模组与配置会单独存放，不影响其它版本。请先在设置里完成一键配置。</p>
                <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => {
                  await containerAttach(d.id);
                  showToast("已为这个版本开启容器隔离");
                  await this.load();
                  await this.refreshLayer(d.id);
                })}>开启容器隔离</button>
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
                title="咦？还没有模组呢…"
                description="去下载中心挑几个装上吧"
              >
                <button class="primary" type="button" @click=${() => this.goDownload("mod")}>去下载模组</button>
              </easymc-empty-state></div>`
            : this.mods.map((m) => {
                const disabled = m.endsWith(".disabled");
                return html`<div class="mod-row">
                  <code>${m}</code>
                  <button class="ghost" type="button" @click=${() => this.act(async () => { await setModDisabled(m, !disabled, d.id); await this.refreshTab(); })}>${disabled ? "启用" : "禁用"}</button>
                  <button class="danger ghost" type="button" @click=${() => this.act(async () => { await removeMod(m, d.id); await this.refreshTab(); showToast("已卸载"); })}>卸载</button>
                </div>`;
              })}
        </div>`;
      case "worlds":
        return this.renderLocals(d, "saves", "世界 / 存档", "咦？还没有存档呢… 进游戏创建世界后会出现在这里");
      case "resourcepacks":
        return this.renderLocals(d, "resourcepacks", "资源包", "咦？还没有资源包呢…", "resourcepack");
      case "shaders":
        return this.renderLocals(d, "shaderpacks", "光影", "咦？还没有光影呢…", "shader");
      case "export":
        return html`<div class="panel">
          <h2>发给好友</h2>
          <p class="muted">导出这个版本的配置，或导入好友发来的分享包。</p>
          <div style="display:flex;gap:8px;flex-wrap:wrap">
            <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => { showToast((await exportInstance(d.id, false)).message_zh); })}>导出轻量包</button>
            <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.act(async () => { showToast((await exportInstance(d.id, true)).message_zh); })}>导出这个版本（含模组）</button>
            <button class="ghost" type="button" @click=${() => this.onImport()}>导入…</button>
          </div>
          <h2 style="margin-top:20px">分享环境包</h2>
          <p class="muted">把模组、配置和加速源偏好打成环境包（不含存档），发给好友导入后会得到一个新的隔离版本。</p>
          <div style="display:flex;gap:8px;flex-wrap:wrap">
            <button class="tonal" type="button" ?disabled=${this.busy || !d.container} @click=${() => this.act(async () => {
              showToast((await containerExportEnv(d.id, { pack: false })).message_zh);
            })}>导出环境元数据</button>
            <button class="ghost" type="button" ?disabled=${this.busy || !d.container} @click=${() => this.act(async () => {
              showToast((await containerExportEnv(d.id, { pack: true })).message_zh);
            })}>导出完整环境包</button>
            <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => this.onImportEnv()}>导入环境包…</button>
          </div>
          ${!d.container ? html`<p class="muted" style="margin-top:8px">这个版本还没开隔离。可先去「设置」开启，或直接导入环境包新建一个。</p>` : nothing}
        </div>`;
    }
  }

  private renderLocals(d: InstanceDto, kind: string, title: string, empty: string, downloadTab?: string) {
    const isEmpty = this.locals.length === 0;
    return html`<div class="panel ${isEmpty ? "panel-empty" : ""}">
      <h2>${title}</h2>
      <div style="display:flex;gap:8px;margin-bottom:10px;flex-wrap:wrap">
        <button class="ghost" type="button" @click=${() => this.act(async () => { showToast(await openInstanceFolder(d.id, kind === "saves" ? "saves" : kind)); })}>打开文件夹</button>
        ${downloadTab ? html`<button class="primary" type="button" @click=${() => this.goDownload(downloadTab)}>去下载</button>` : nothing}
      </div>
      ${isEmpty
        ? html`<div class="panel-empty-slot"><easymc-empty-state
            variant="panel"
            title=${empty}
            description=""
          ></easymc-empty-state></div>`
        : this.locals.map((e) => html`<div class="local-row">
            <span>${e.name}${e.is_dir ? "/" : ""} <span class="muted">${Math.round(e.size / 1024)} KB</span></span>
            <button class="danger ghost" type="button" @click=${() => this.act(async () => {
              if (!confirm(`删除 ${e.name}？`)) return;
              await deleteLocalContent(d.id, kind, e.name);
              await this.refreshTab();
            })}>删除</button>
          </div>`)}
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
