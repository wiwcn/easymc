import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import type { InstanceDto, VersionSummary } from "../types";
import { versionId, versionType } from "../types";
import {
  listInstancesState,
  listVersions,
  createInstance,
  setActiveInstance,
  installVersion,
  installLoaderVersion,
  listLoaderVersions,
  ensureJava,
  listenInstallProgress,
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

@customElement("easymc-library-page")
export class LibraryPage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private loading = true;
  @state() private busy = false;
  @state() private status = "";

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

  private unProg?: () => void;

  static styles = css`
    :host {
      display: block;
    }
    .head {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 12px;
      margin-bottom: 16px;
      flex-wrap: wrap;
    }
    h1 {
      margin: 0;
      font-size: 1.35rem;
      font-weight: 700;
    }
    .sub {
      margin: 4px 0 0;
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.9rem;
    }
    .btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
      padding: 10px 18px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.9rem;
      font-weight: 600;
      cursor: pointer;
    }
    .btn:disabled {
      opacity: 0.55;
      cursor: not-allowed;
    }
    .btn.primary {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .btn.ghost {
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface);
    }
    .btn.sm {
      padding: 6px 12px;
      font-size: 0.8rem;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
      gap: 14px;
    }
    .card {
      padding: 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      display: flex;
      flex-direction: column;
      gap: 8px;
      border: 2px solid transparent;
    }
    .card.active {
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
    }
    .card h3 {
      margin: 0;
      font-size: 1.05rem;
    }
    .meta {
      font-size: 0.82rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .badge {
      display: inline-block;
      padding: 2px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.72rem;
      font-weight: 600;
      background: var(--md-sys-color-surface-container-highest);
    }
    .badge.ok {
      background: #c8e6c9;
      color: #1b5e20;
    }
    .badge.warn {
      background: #ffe0b2;
      color: #e65100;
    }
    .badge.def {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .row {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
      margin-top: auto;
      padding-top: 8px;
    }
    .status {
      margin: 0 0 14px;
      font-size: 0.9rem;
      color: var(--md-sys-color-on-surface-variant);
      min-height: 1.2em;
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
    input[type="text"],
    select {
      padding: 10px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
    }
    .toolbar {
      display: flex;
      gap: 10px;
      flex-wrap: wrap;
      align-items: center;
    }
    .check {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      font-size: 0.85rem;
      font-weight: 500;
      color: var(--md-sys-color-on-surface-variant);
      cursor: pointer;
      flex-direction: row;
    }
    .ver-list {
      max-height: 220px;
      overflow: auto;
      border: 1px solid var(--md-sys-color-outline-variant);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface);
    }
    .ver-item {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      width: 100%;
      padding: 10px 14px;
      border: none;
      border-bottom: 1px solid var(--md-sys-color-outline-variant);
      background: transparent;
      text-align: left;
      cursor: pointer;
      font: inherit;
      color: var(--md-sys-color-on-surface);
    }
    .ver-item:last-child {
      border-bottom: none;
    }
    .ver-item:hover {
      background: var(--md-sys-color-surface-container);
    }
    .ver-item.selected {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
    }
    .ver-item .right {
      display: flex;
      gap: 6px;
      align-items: center;
      font-size: 0.75rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .empty {
      padding: 24px;
      text-align: center;
      color: var(--md-sys-color-on-surface-variant);
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
    void listenInstallProgress((p) => {
      this.progress = p;
      this.status = formatProgressLine(p);
    }).then((u) => (this.unProg = u));
  }

  disconnectedCallback() {
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

  private async openCreate() {
    this.showCreate = true;
    this.newName = "";
    this.newVersion = "1.21.1";
    this.newLoader = "vanilla";
    this.newLoaderVersion = "";
    this.loaderVersions = [];
    this.versionFilter = "";
    await this.refreshVersions();
  }

  private async refreshVersions() {
    this.versionsLoading = true;
    try {
      this.versions = await listVersions(this.includeSnapshots);
      if (!this.versions.find((v) => v.id === this.newVersion) && this.versions[0]) {
        this.newVersion = this.versions[0].id;
      }
    } catch (e) {
      showToast(String(e));
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
    const version = this.newVersion;
    if (!version) {
      showToast("请选择版本");
      return;
    }
    this.busy = true;
    this.status = `正在创建实例「${name}」…`;
    try {
      const inst = await createInstance(name, version, this.newLoader, this.newLoaderVersion || null);
      showToast(`已创建 ${inst.name}`);
      if (this.alsoInstall && !inst.version_ready) {
        this.progress = null;
        const loaderTip =
          this.newLoader === "vanilla" ? "" : ` · ${loaderLabel(this.newLoader)}`;
        this.status = `正在安装 ${version}${loaderTip}…`;
        await ensureJava(21);
        const msg =
          this.newLoader === "vanilla"
            ? await installVersion(version, this.withAssets)
            : await installLoaderVersion(
                version,
                this.newLoader,
                this.newLoaderVersion || null,
                this.withAssets,
              );
        showToast(msg);
        this.status = msg;
      } else if (this.alsoInstall && inst.version_ready) {
        this.status = `已创建 · ${loaderLabel(this.newLoader)} ${versionId(inst.version)} 已在本地，跳过下载`;
      } else {
        this.status = `已创建 · ${versionId(inst.version)} · ${loaderLabel(this.newLoader)}`;
      }
      this.showCreate = false;
      await this.load();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }

  private async onSetActive(id: string) {
    try {
      const state = await setActiveInstance(id);
      this.instances = state.instances;
      this.activeId = state.active_id;
      showToast("已设为默认实例");
    } catch (e) {
      showToast(String(e));
    }
  }

  private async onInstall(inst: InstanceDto, withAssets = this.withAssets) {
    const ver = versionId(inst.version);
    this.busy = true;
    this.progress = null;
    const mode = withAssets ? "含完整资源" : "跳过资源";
    this.status = `正在安装 ${ver}（${mode}）…`;
    try {
      await ensureJava(21);
      const msg = await installVersion(ver, withAssets);
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


  private async onExport(inst: InstanceDto, pack: boolean) {
    this.busy = true;
    this.status = pack ? "正在导出整合包…" : "正在导出元数据…";
    try {
      const res = await exportInstance(inst.id, pack);
      this.status = res.message_zh;
      showToast(res.path);
      try {
        await navigator.clipboard.writeText(res.path);
        showToast("路径已复制到剪贴板");
      } catch {
        /* clipboard may be unavailable */
      }
    } catch (e) {
      this.status = String(e);
      showToast(String(e));
    } finally {
      this.busy = false;
    }
  }

  private async onImport() {
    const path = window.prompt(
      "导入分享包路径（.json 元数据 或 .zip 整合包）",
      "",
    );
    if (!path?.trim()) return;
    const applyMem = window.confirm("是否将分享包中的内存建议写入本地 settings？");
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
      this.status = String(e);
      showToast(String(e));
    } finally {
      this.busy = false;
    }
  }

  render() {
    if (this.loading) return html`<p>加载中…</p>`;
    return html`
      <div class="head">
        <div>
          <h1>库</h1>
          <p class="sub">管理实例 · 选择版本创建新世界</p>
        </div>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button class="btn ghost" type="button" ?disabled=${this.busy} @click=${() => this.onImport()}>
            导入分享
          </button>
          <button class="btn primary" type="button" ?disabled=${this.busy} @click=${() => this.openCreate()}>
            ＋ 新建实例
          </button>
        </div>
      </div>

      <p class="status">${this.status}</p>
      ${this.renderProgress()}

      ${this.showCreate ? this.renderCreate() : null}

      <div class="grid">
        ${this.instances.map((inst) => this.renderCard(inst))}
        ${this.instances.length === 0
          ? html`<div class="empty">还没有实例，点击「新建实例」开始。</div>`
          : null}
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
            placeholder="例如：生存 1.21"
            @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)}
          />
        </label>
        <div class="toolbar">
          <label style="flex:1;min-width:160px">
            筛选版本
            <input
              type="text"
              .value=${this.versionFilter}
              placeholder="搜索 1.20 / release…"
              @input=${(e: Event) => (this.versionFilter = (e.target as HTMLInputElement).value)}
            />
          </label>
          <label class="check">
            <input
              type="checkbox"
              .checked=${this.includeSnapshots}
              @change=${async (e: Event) => {
                this.includeSnapshots = (e.target as HTMLInputElement).checked;
                await this.refreshVersions();
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
            创建后立即安装
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
        <div>
          <div class="meta" style="margin-bottom:6px">
            已选版本：<strong>${this.newVersion}</strong>
            ${this.versionsLoading ? " · 加载中…" : ` · ${list.length} 项`}
          </div>
          <div class="ver-list">
            ${list.length === 0
              ? html`<div class="empty">${this.versionsLoading ? "加载版本列表…" : "无匹配版本"}</div>`
              : list.map(
                  (v) => html`
                    <button
                      type="button"
                      class="ver-item ${v.id === this.newVersion ? "selected" : ""}"
                      @click=${() => { this.newVersion = v.id; void this.refreshLoaderVersions(); }}
                    >
                      <span>${v.id}</span>
                      <span class="right">
                        <span>${versionType(v)}</span>
                        ${v.installed ? html`<span class="badge ok">已安装</span>` : null}
                      </span>
                    </button>
                  `,
                )}
          </div>
        </div>
        <div class="row">
          <button class="btn primary" type="button" ?disabled=${this.busy} @click=${() => this.onCreate()}>
            创建
          </button>
          <button class="btn ghost" type="button" ?disabled=${this.busy} @click=${() => (this.showCreate = false)}>
            取消
          </button>
        </div>
      </section>
    `;
  }

  private renderCard(inst: InstanceDto) {
    const ver = versionId(inst.version);
    const isActive = inst.id === this.activeId;
    return html`
      <article class="card ${isActive ? "active" : ""}">
        <div>
          ${isActive ? html`<span class="badge def">默认</span>` : null}
          ${inst.version_ready
            ? html`<span class="badge ok">已安装</span>`
            : html`<span class="badge warn">未安装</span>`}
        </div>
        <h3>${inst.name}</h3>
        <div class="meta">${ver} · ${loaderLabel(inst.loader)}${inst.loader_version ? ` ${inst.loader_version}` : ""}</div>
        <div class="meta">${inst.description ?? ""}</div>
        <div class="row">
          ${!isActive
            ? html`<button class="btn ghost sm" type="button" @click=${() => this.onSetActive(inst.id)}>
                设为默认
              </button>`
            : null}
          ${!inst.version_ready
            ? html`
                <button
                  class="btn primary sm"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.onInstall(inst, true)}
                >
                  安装 ${ver}
                </button>
                <button
                  class="btn ghost sm"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.onInstall(inst, false)}
                >
                  快速（无资源）
                </button>
              `
            : html`<span class="badge ok">本地已有版本文件</span>`}
          <button
            class="btn ghost sm"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onExport(inst, false)}
          >
            导出元数据
          </button>
          <button
            class="btn ghost sm"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onExport(inst, true)}
          >
            导出整合包
          </button>
        </div>
      </article>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-library-page": LibraryPage;
  }
}
