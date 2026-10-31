import { LitElement, css, html } from "lit";
import { controlStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  curseforgeConfigured,
  installMod,
  listInstancesState,
  listModVersions,
  listenInstallProgress,
  searchMods,
  setCurseforgeKey,
} from "../api";
import {
  formatProgressLine,
  overallPercent,
  type InstallProgressDto,
} from "../install-progress";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import type {
  CurseForgeStatusDto,
  InstanceDto,
  ModProjectDto,
  ModSourceId,
  ModVersionDto,
} from "../types";
import { loaderLabel, versionId } from "../types";
import "../components/empty-state";

@customElement("easymc-store-page")
export class StorePage extends LitElement {
  @state() private source: ModSourceId = "modrinth";
  @state() private query = "";
  @state() private results: ModProjectDto[] = [];
  @state() private loading = true;
  @state() private error = "";
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private progress: InstallProgressDto | null = null;
  @state() private installingId: string | null = null;
  @state() private expandedId: string | null = null;
  @state() private versions: ModVersionDto[] = [];
  @state() private versionsLoading = false;
  @state() private cfKeyDraft = "";

  private unlistenProgress: (() => void) | null = null;

  static styles = [
    controlStyles,
    css`
    :host {
      display: flex;
      flex-direction: column;
      flex: 1 0 auto;
      min-height: 100%;
      width: 100%;
      max-width: 960px;
      box-sizing: border-box;
    }
    h1 {
      margin: 0 0 4px;
      font-size: var(--easymc-type-display);
    }
    .sub {
      margin: 0 0 16px;
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-body);
    }
    .toolbar {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
      align-items: center;
      margin-bottom: var(--easymc-space-3);
    }
    .chip {
      min-height: 32px;
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
    /* primary / ghost → controlStyles (CMP-1); size tweak */
    button.primary { padding: 10px 18px; }
    .meta {
      font-size: var(--easymc-type-label);
      color: var(--md-sys-color-on-surface-variant);
      margin-bottom: 12px;
    }
    .warn {
      padding: 12px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
      margin-bottom: 12px;
      line-height: 1.45;
    }
    .err {
      padding: 12px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-error-container);
      color: var(--md-sys-color-on-error-container);
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
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr));
      gap: var(--easymc-grid-gap);
      flex: 1 1 auto;
      min-height: 220px;
      align-content: start;
    }
    @container easymc-page (max-width: 720px) {
      .list {
        grid-template-columns: repeat(
          auto-fill,
          minmax(min(var(--easymc-grid-min-dense-narrow), 100%), 1fr)
        );
      }
    }
    .results-slot {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 220px;
    }
    .card {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-3);
      padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1); /* CF-1 */
      min-height: 0;
    }
    .card-top {
      display: flex;
      gap: var(--easymc-space-3);
      align-items: flex-start;
    }
    .icon {
      width: 56px;
      height: 56px;
      border-radius: var(--md-sys-shape-corner-large);
      object-fit: cover;
      background: var(--md-sys-color-primary-container);
      flex-shrink: 0;
    }
    .icon.placeholder {
      display: grid;
      place-items: center;
      font-size: var(--easymc-type-title);
      color: var(--md-sys-color-on-primary-container);
    }
    .title {
      font-weight: var(--easymc-weight-medium);
      margin: 0 0 4px;
      font-size: var(--easymc-type-body);
    }
    .desc {
      margin: 0;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.4;
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
    }
    .stats {
      margin-top: 6px;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      align-items: center;
      margin-top: auto;
    }
    .versions {
      border-top: 1px solid var(--md-sys-color-outline-variant);
      padding-top: 10px;
      margin-top: 4px;
    }
    .versions select {
      width: 100%;
      padding: 8px 10px;
      border-radius: var(--md-sys-shape-corner-small); /* CMP-5: form field */
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      margin-bottom: 8px;
    }
    .cf-box {
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-warning-container);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-warning) 35%, transparent);
      margin-bottom: 12px;
      line-height: 1.45;
    }
    .cf-box a {
      color: var(--md-sys-color-on-warning-container);
    }
    .cf-box .row {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-top: 10px;
    }
    .cf-box input {
      flex: 1;
      min-width: 180px;
      padding: 8px 12px;
      border-radius: var(--md-sys-shape-corner-small); /* CMP-5: form field */
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
    }
    code {
      font-size: var(--easymc-type-meta);
      background: var(--md-sys-color-surface-container-highest);
      padding: 1px 5px;
      border-radius: 4px;
    }

    button.primary,
    button.ghost,
    .chip {
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        opacity var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.primary:active:not(:disabled),
    button.ghost:active:not(:disabled),
    .chip:active {
      transform: scale(0.98);
    }
    .hint {
      margin-top: 20px;
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-label);
      line-height: 1.45;
    }
    .hint button {
      margin-left: 6px;
      padding: 0;
      border: none;
      background: none;
      color: var(--md-sys-color-primary);
      font: inherit;
      font-weight: var(--easymc-weight-medium);
      cursor: pointer;
      text-decoration: underline;
      text-underline-offset: 2px;
    }
    .warn {
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
    }
    .err {
      background: var(--md-sys-color-error-container);
      color: var(--md-sys-color-on-error-container);
    }
    .cf-box {
      background: var(--md-sys-color-warning-container);
      border-color: color-mix(in srgb, var(--md-sys-color-warning) 35%, transparent);
      color: var(--md-sys-color-on-warning-container);
    }
    .cf-box a {
      color: var(--md-sys-color-warning);
    }
    @media (prefers-reduced-motion: reduce) {
      button.primary:active:not(:disabled),
      button.ghost:active:not(:disabled),
      .chip:active { transform: none; }
    }
  `,
  ];

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
    await this.onSearch(); // empty query → popular/hot
  }

  private async onSaveCfKey() {
    try {
      const res = await setCurseforgeKey(this.cfKeyDraft.trim() || null);
      showToast(res.message);
      this.cf = await curseforgeConfigured();
      this.cfKeyDraft = "";
    } catch (e) {
      showToast(friendlyError(e));
    }
  }

  private async onClearCfKey() {
    try {
      const res = await setCurseforgeKey(null);
      showToast(res.message);
      this.cf = await curseforgeConfigured();
    } catch (e) {
      showToast(friendlyError(e));
    }
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
      // Empty results use EmptyState (not error banner); keep this.error for failures only.
    } catch (e) {
      this.error = friendlyError(e);
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
      showToast(friendlyError(e));
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
          : `已安装 ${res.project_name} → ${res.filename}（可在「库」管理）`,
      );
    } catch (e) {
      const msg = friendlyError(e);
      this.error = msg;
      showToast(msg);
    } finally {
      this.installingId = null;
    }
  }

  private goLibrary() {
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: "library", bubbles: true, composed: true }),
    );
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
      <p class="sub">搜索并安装到当前实例。安装后的启用 / 禁用 / 卸载请到「库」对应实例。</p>

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
          placeholder="搜索模组…（留空显示热门）"
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
        装进哪个版本：
        ${inst
          ? html`<strong>${inst.name}</strong> · ${versionId(inst.version)} ·
              ${loaderLabel(inst.loader)}`
          : "（还没有版本——先去下载装一个）"}
        · 列表会按当前版本与加载器筛选
      </div>

      ${this.source === "curseforge" && this.cf && !this.cf.configured
        ? html`<div class="cf-box">
            <strong>浏览 CurseForge 需要密钥（可选）</strong>
            <p style="margin:6px 0 0">
              在
              <a href=${this.cf.console_url || "https://console.curseforge.com/"} target="_blank" rel="noreferrer"
                >CurseForge Console</a
              >
              申请后粘贴到下方，只保存在这台电脑上。Modrinth 无需密钥。
            </p>
            <div class="row">
              <input
                type="password"
                autocomplete="off"
                placeholder="粘贴 CurseForge 密钥…"
                .value=${this.cfKeyDraft}
                @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)}
              />
              <button class="primary" type="button" @click=${() => this.onSaveCfKey()}>保存</button>
            </div>
          </div>`
        : this.source === "curseforge" && this.cf?.configured
          ? html`<div class="meta">
              CurseForge 已可用
            </div>`
          : null}

      ${this.error ? html`<div class="err">${this.error}</div>` : null}

      ${this.progress
        ? html`<div class="progress-card">
            <div>${formatProgressLine(this.progress)}</div>
            <div class="bar"><span style="width:${pct ?? 0}%"></span></div>
          </div>`
        : null}

      ${!this.loading && !this.error && !this.results.length
        ? html`<div class="results-slot">
            <easymc-empty-state
              variant="panel"
              title=${this.query.trim() ? "咦？好像什么都没搜到…" : "咦？热门列表空空的…"}
              description=${this.query.trim()
                ? "换个关键词试试，或者清空后再看热门"
                : "稍后再试，或换个来源看看"}
            >
              ${this.query.trim()
                ? html`<button class="ghost" type="button" @click=${() => { this.query = ""; void this.onSearch(); }}>清空搜索</button>`
                : null}
            </easymc-empty-state>
          </div>`
        : html`<div class="list">
            ${this.results.map((p) => this.renderCard(p))}
          </div>`}

      <p class="hint">
        已装好的模组可在「版本管理」里启用、禁用或卸载。
        <button type="button" @click=${() => this.goLibrary()}>打开版本管理</button>
      </p>
    `;
  }

  private renderCard(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const expanded = this.expandedId === p.id;
    return html`
      <div class="card">
        <div class="card-top">
          ${p.icon_url
            ? html`<img class="icon" src=${p.icon_url} alt="" loading="lazy" />`
            : html`<div class="icon placeholder" aria-hidden="true">M</div>`}
          <div>
            <p class="title">${p.name}</p>
            <p class="desc">${p.description}</p>
            <div class="stats">
              ${p.source} · ↓ ${this.formatDl(p.downloads)}
              ${p.author ? html` · ${p.author}` : null}
            </div>
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
                  : html`<p class="meta">咦？这个模组暂时没有合适的版本… 换个游戏版本或加载器再试试</p>`}
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
