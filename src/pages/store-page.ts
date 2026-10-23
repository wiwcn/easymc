import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import {
  curseforgeConfigured,
  installMod,
  listInstalledMods,
  listInstancesState,
  listModVersions,
  listenInstallProgress,
  searchMods,
} from "../api";
import {
  formatProgressLine,
  overallPercent,
  type InstallProgressDto,
} from "../install-progress";
import { showToast } from "../toast";
import type {
  CurseForgeStatusDto,
  InstanceDto,
  ModProjectDto,
  ModSourceId,
  ModVersionDto,
} from "../types";
import { loaderLabel, versionId } from "../types";

@customElement("easymc-store-page")
export class StorePage extends LitElement {
  @state() private source: ModSourceId = "modrinth";
  @state() private query = "";
  @state() private results: ModProjectDto[] = [];
  @state() private loading = false;
  @state() private error = "";
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private installed: string[] = [];
  @state() private progress: InstallProgressDto | null = null;
  @state() private installingId: string | null = null;
  @state() private expandedId: string | null = null;
  @state() private versions: ModVersionDto[] = [];
  @state() private versionsLoading = false;

  private unlistenProgress: (() => void) | null = null;

  static styles = css`
    :host {
      display: block;
      max-width: 960px;
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
    .toolbar {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
      margin-bottom: 12px;
    }
    .chip {
      padding: 8px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      font: inherit;
    }
    .chip.on {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border-color: transparent;
    }
    input[type="search"] {
      flex: 1;
      min-width: 180px;
      padding: 10px 16px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      background: var(--md-sys-color-surface-container-lowest);
    }
    button.primary {
      padding: 10px 18px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      font-weight: 600;
      cursor: pointer;
    }
    button.primary:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
    button.ghost {
      padding: 8px 14px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      font: inherit;
    }
    .meta {
      font-size: 0.85rem;
      color: var(--md-sys-color-on-surface-variant);
      margin-bottom: 12px;
    }
    .warn {
      padding: 12px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: #fff3e0;
      color: #e65100;
      margin-bottom: 12px;
      line-height: 1.45;
    }
    .err {
      padding: 12px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: #ffebee;
      color: #b71c1c;
      margin-bottom: 12px;
    }
    .progress-card {
      padding: 12px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      margin-bottom: 12px;
      box-shadow: var(--md-sys-elevation-1);
    }
    .bar {
      height: 8px;
      border-radius: 999px;
      background: var(--md-sys-color-surface-container-highest);
      overflow: hidden;
      margin-top: 8px;
    }
    .bar > span {
      display: block;
      height: 100%;
      background: var(--md-sys-color-primary);
      border-radius: 999px;
      transition: width 0.15s ease;
    }
    .list {
      display: flex;
      flex-direction: column;
      gap: 10px;
    }
    .card {
      display: grid;
      grid-template-columns: 48px 1fr auto;
      gap: 12px;
      align-items: start;
      padding: 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
    }
    .icon {
      width: 48px;
      height: 48px;
      border-radius: 12px;
      object-fit: cover;
      background: var(--md-sys-color-surface-container-highest);
    }
    .icon.placeholder {
      display: grid;
      place-items: center;
      font-size: 1.4rem;
    }
    .title {
      font-weight: 600;
      margin: 0 0 4px;
    }
    .desc {
      margin: 0;
      font-size: 0.9rem;
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.4;
    }
    .stats {
      margin-top: 6px;
      font-size: 0.8rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .actions {
      display: flex;
      flex-direction: column;
      gap: 6px;
      align-items: stretch;
    }
    .versions {
      grid-column: 1 / -1;
      border-top: 1px solid var(--md-sys-color-outline-variant);
      padding-top: 10px;
      margin-top: 4px;
    }
    .versions select {
      width: 100%;
      padding: 8px 10px;
      border-radius: 8px;
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      margin-bottom: 8px;
    }
    .side {
      margin-top: 20px;
      padding: 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container);
    }
    .side h2 {
      margin: 0 0 8px;
      font-size: 1rem;
    }
    .side ul {
      margin: 0;
      padding-left: 18px;
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.9rem;
    }
    code {
      font-size: 0.85em;
      background: var(--md-sys-color-surface-container-highest);
      padding: 1px 5px;
      border-radius: 4px;
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.bootstrap();
    void listenInstallProgress((p) => {
      this.progress = p;
    }).then((un) => {
      this.unlistenProgress = un;
    });
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    this.unlistenProgress?.();
  }

  private get active(): InstanceDto | undefined {
    return (
      this.instances.find((i) => i.id === this.activeId) ?? this.instances[0]
    );
  }

  private async bootstrap() {
    const state = await listInstancesState();
    this.instances = state.instances;
    this.activeId = state.active_id;
    this.cf = await curseforgeConfigured();
    await this.refreshInstalled();
  }

  private async refreshInstalled() {
    const id = this.active?.id;
    this.installed = await listInstalledMods(id ?? null);
  }

  private async onSearch() {
    this.error = "";
    this.loading = true;
    this.results = [];
    this.expandedId = null;
    try {
      if (this.source === "curseforge" && this.cf && !this.cf.configured) {
        this.error = this.cf.hint;
        return;
      }
      const inst = this.active;
      const game = inst ? versionId(inst.version) : "1.21.1";
      const loader = inst?.loader ?? "fabric";
      this.results = await searchMods(
        this.source,
        this.query.trim(),
        game,
        loader === "vanilla" ? null : loader,
        20,
      );
      if (!this.results.length) {
        this.error = "无结果。可换关键词或检查实例版本/加载器筛选。";
      }
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
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
    this.versions = [];
    this.versionsLoading = true;
    try {
      const inst = this.active;
      const game = inst ? versionId(inst.version) : undefined;
      const loader = inst?.loader;
      this.versions = await listModVersions(
        p.source,
        p.id,
        game,
        loader && loader !== "vanilla" ? loader : null,
      );
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.versionsLoading = false;
    }
  }

  private async onInstall(p: ModProjectDto, versionId?: string | null) {
    if (!this.active) {
      showToast("请先在「库」中设置活动实例");
      return;
    }
    this.installingId = p.id;
    this.progress = null;
    this.error = "";
    try {
      const res = await installMod(p.source, p.id, versionId ?? null, this.active.id);
      showToast(
        res.skipped
          ? `已存在 ${res.filename}`
          : `已安装 ${res.project_name} → ${res.filename}`,
      );
      await this.refreshInstalled();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.error = msg;
      showToast(msg);
    } finally {
      this.installingId = null;
    }
  }

  private setSource(s: ModSourceId) {
    this.source = s;
    this.error = "";
    this.results = [];
  }

  private formatDl(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
    return String(n);
  }

  render() {
    const inst = this.active;
    const pct = this.progress ? overallPercent(this.progress) : null;
    return html`
      <h1>商店</h1>
      <p class="sub">从 Modrinth / CurseForge 搜索并安装模组到当前实例的 mods 目录。</p>

      <div class="toolbar">
        <button
          class="chip ${this.source === "modrinth" ? "on" : ""}"
          type="button"
          @click=${() => this.setSource("modrinth")}
        >
          Modrinth
        </button>
        <button
          class="chip ${this.source === "curseforge" ? "on" : ""}"
          type="button"
          @click=${() => this.setSource("curseforge")}
        >
          CurseForge
        </button>
        <input
          type="search"
          placeholder="搜索模组…"
          .value=${this.query}
          @input=${(e: Event) => (this.query = (e.target as HTMLInputElement).value)}
          @keydown=${(e: KeyboardEvent) => {
            if (e.key === "Enter") void this.onSearch();
          }}
        />
        <button class="primary" type="button" ?disabled=${this.loading} @click=${() => this.onSearch()}>
          ${this.loading ? "搜索中…" : "搜索"}
        </button>
      </div>

      <div class="meta">
        目标实例：
        ${inst
          ? html`<strong>${inst.name}</strong> · ${versionId(inst.version)} ·
              ${loaderLabel(inst.loader)} · <code>${inst.id}</code>`
          : "（无实例）"}
        · 筛选会按实例版本与加载器过滤
      </div>

      ${this.source === "curseforge" && this.cf && !this.cf.configured
        ? html`<div class="warn">${this.cf.hint}</div>`
        : null}

      ${this.error ? html`<div class="err">${this.error}</div>` : null}

      ${this.progress
        ? html`<div class="progress-card">
            <div>${formatProgressLine(this.progress)}</div>
            <div class="bar"><span style="width:${pct ?? 0}%"></span></div>
          </div>`
        : null}

      <div class="list">
        ${this.results.map((p) => this.renderCard(p))}
      </div>

      <div class="side">
        <h2>已安装（当前实例 mods/）</h2>
        ${this.installed.length
          ? html`<ul>
              ${this.installed.map((m) => html`<li>${m}</li>`)}
            </ul>`
          : html`<p class="meta">暂无 jar</p>`}
        <button class="ghost" type="button" @click=${() => this.refreshInstalled()}>刷新列表</button>
      </div>
    `;
  }

  private renderCard(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const expanded = this.expandedId === p.id;
    return html`
      <div class="card">
        ${p.icon_url
          ? html`<img class="icon" src=${p.icon_url} alt="" />`
          : html`<div class="icon placeholder">📦</div>`}
        <div>
          <p class="title">${p.name}</p>
          <p class="desc">${p.description}</p>
          <div class="stats">
            ${p.source} · 下载 ${this.formatDl(p.downloads)}
            ${p.author ? html` · ${p.author}` : null}
            · <code>${p.slug || p.id}</code>
          </div>
        </div>
        <div class="actions">
          <button
            class="primary"
            type="button"
            ?disabled=${busy || (this.source === "curseforge" && !!this.cf && !this.cf.configured)}
            @click=${() => this.onInstall(p)}
          >
            ${busy ? "安装中…" : "安装最新"}
          </button>
          <button class="ghost" type="button" @click=${() => this.onExpand(p)}>
            ${expanded ? "收起版本" : "选版本"}
          </button>
        </div>
        ${expanded
          ? html`<div class="versions">
              ${this.versionsLoading
                ? html`<p class="meta">加载版本…</p>`
                : this.versions.length
                  ? html`
                      <select
                        id="ver-${p.id}"
                        @change=${(e: Event) => {
                          const sel = e.target as HTMLSelectElement;
                          void this.onInstall(p, sel.value);
                        }}
                      >
                        <option value="">选择版本安装…</option>
                        ${this.versions.map(
                          (v) => html`<option value=${v.id}>
                            ${v.version_number} · ${v.name}
                            (${v.game_versions.slice(0, 3).join(", ")}${v.game_versions.length > 3
                              ? "…"
                              : ""})
                          </option>`,
                        )}
                      </select>
                    `
                  : html`<p class="meta">无匹配版本</p>`}
            </div>`
          : null}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-store-page": StorePage;
  }
}
