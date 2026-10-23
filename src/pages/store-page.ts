import { LitElement, css, html } from "lit";
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
  @state() private progress: InstallProgressDto | null = null;
  @state() private installingId: string | null = null;
  @state() private expandedId: string | null = null;
  @state() private versions: ModVersionDto[] = [];
  @state() private versionsLoading = false;
  @state() private cfKeyDraft = "";

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
    .cf-box {
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: #fff8e1;
      border: 1px solid #ffe082;
      margin-bottom: 12px;
      line-height: 1.45;
    }
    .cf-box a {
      color: #e65100;
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
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
    }
    code {
      font-size: 0.85em;
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
    .card {
      animation: row-in var(--easymc-dur-med) var(--easymc-ease-out) both;
    }
    .list .card:nth-child(1) { animation-delay: 0ms; }
    .list .card:nth-child(2) { animation-delay: 30ms; }
    .list .card:nth-child(3) { animation-delay: 60ms; }
    .list .card:nth-child(4) { animation-delay: 90ms; }
    .list .card:nth-child(5) { animation-delay: 120ms; }
    .list .card:nth-child(n+6) { animation-delay: 140ms; }
    @keyframes row-in {
      from { opacity: 0; transform: translateY(8px); }
      to { opacity: 1; transform: none; }
    }
    .hint {
      margin-top: 20px;
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
      font-size: 0.9rem;
      line-height: 1.45;
    }
    .hint button {
      margin-left: 6px;
      padding: 0;
      border: none;
      background: none;
      color: var(--md-sys-color-primary);
      font: inherit;
      font-weight: 600;
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
      .card { animation: none; }
      button.primary:active:not(:disabled),
      button.ghost:active:not(:disabled),
      .chip:active { transform: none; }
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
  }

  private async onSaveCfKey() {
    try {
      const res = await setCurseforgeKey(this.cfKeyDraft.trim() || null);
      showToast(res.message);
      this.cf = await curseforgeConfigured();
      this.cfKeyDraft = "";
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    }
  }

  private async onClearCfKey() {
    try {
      const res = await setCurseforgeKey(null);
      showToast(res.message);
      this.cf = await curseforgeConfigured();
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
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
          : `已安装 ${res.project_name} → ${res.filename}（可在「库」管理）`,
      );
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
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
        ? html`<div class="cf-box">
            <strong>需要 CurseForge API Key</strong>
            <p style="margin:6px 0 0">
              在
              <a href=${this.cf.console_url || "https://console.curseforge.com/"} target="_blank" rel="noreferrer"
                >CurseForge Console</a
              >
              申请后粘贴到下方（保存在本地设置，不会提交到 git）。也可设置环境变量
              <code>${this.cf.env}</code>。Modrinth 无需 Key。
            </p>
            <div class="row">
              <input
                type="password"
                autocomplete="off"
                placeholder="粘贴 CurseForge API Key…"
                .value=${this.cfKeyDraft}
                @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)}
              />
              <button class="primary" type="button" @click=${() => this.onSaveCfKey()}>保存 Key</button>
            </div>
          </div>`
        : this.source === "curseforge" && this.cf?.configured
          ? html`<div class="meta">
              CurseForge Key 已配置（来源 ${this.cf.source ?? "?"}
              ${this.cf.key_len ? html` · 长度 ${this.cf.key_len}` : null}）
            </div>`
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

      <p class="hint">
        已安装模组在「库」中按实例管理（启用 / 禁用 / 卸载）。
        <button type="button" @click=${() => this.goLibrary()}>打开库</button>
      </p>
    `;
  }

  private renderCard(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const expanded = this.expandedId === p.id;
    return html`
      <div class="card">
        ${p.icon_url
          ? html`<img class="icon" src=${p.icon_url} alt="" />`
          : html`<div class="icon placeholder" aria-hidden="true">M</div>`}
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
