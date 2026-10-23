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
  ensureJava,
  listenInstallProgress,
} from "../api";
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
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.load();
    void listenInstallProgress((p) => {
      const tot = p.total != null ? `/${p.total}` : "";
      this.status = `[${p.stage}] ${p.current}${tot} ${p.detail}`;
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
      const inst = await createInstance(name, version);
      showToast(`已创建 ${inst.name}`);
      if (this.alsoInstall && !inst.version_ready) {
        this.status = `正在安装 ${version}…`;
        await ensureJava(21);
        const msg = await installVersion(version, true);
        showToast(msg);
        this.status = msg;
      } else {
        this.status = `已创建 · ${versionId(inst.version)}`;
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

  private async onInstall(inst: InstanceDto) {
    const ver = versionId(inst.version);
    this.busy = true;
    this.status = `正在安装 ${ver}…`;
    try {
      await ensureJava(21);
      const msg = await installVersion(ver, true);
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

  render() {
    if (this.loading) return html`<p>加载中…</p>`;
    return html`
      <div class="head">
        <div>
          <h1>库</h1>
          <p class="sub">管理实例 · 选择版本创建新世界</p>
        </div>
        <button class="btn primary" type="button" ?disabled=${this.busy} @click=${() => this.openCreate()}>
          ＋ 新建实例
        </button>
      </div>

      <p class="status">${this.status}</p>

      ${this.showCreate ? this.renderCreate() : null}

      <div class="grid">
        ${this.instances.map((inst) => this.renderCard(inst))}
        ${this.instances.length === 0
          ? html`<div class="empty">还没有实例，点击「新建实例」开始。</div>`
          : null}
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
          <label class="check">
            <input
              type="checkbox"
              .checked=${this.alsoInstall}
              @change=${(e: Event) => (this.alsoInstall = (e.target as HTMLInputElement).checked)}
            />
            创建后立即安装
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
                      @click=${() => (this.newVersion = v.id)}
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
        <div class="meta">${ver} · ${inst.loader}</div>
        <div class="meta">${inst.description ?? ""}</div>
        <div class="row">
          ${!isActive
            ? html`<button class="btn ghost sm" type="button" @click=${() => this.onSetActive(inst.id)}>
                设为默认
              </button>`
            : null}
          ${!inst.version_ready
            ? html`<button
                class="btn primary sm"
                type="button"
                ?disabled=${this.busy}
                @click=${() => this.onInstall(inst)}
              >
                安装 ${ver}
              </button>`
            : null}
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
