import { LitElement, css, html, nothing } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import { controlStyles, pageChromeStyles, cardSurfaceStyles } from "../styles/controls";
import {
  wardrobeImportLittleSkin,
  wardrobeImportOfficial,
  wardrobeList,
  wardrobeRemove,
  wardrobeReplace,
  wardrobeSetActive,
  wardrobeUpload,
  skinFetchLittleSkin,
  skinFetchOfficial,
} from "../api";
import type { ResolvedSkinDto, WardrobeEntryDto } from "../types";
import { pickSkinPng } from "../lib/pick-file";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import "../components/skin-viewer";
import "../components/empty-state";

type Tab = "mine" | "official" | "littleskin";

@customElement("easymc-wardrobe-page")
export class WardrobePage extends LitElement {
  /** When true (entered from accounts), show optional「← 账户」chrome. */
  @property({ type: Boolean }) fromAccounts = false;

  @state() private tab: Tab = "mine";
  @state() private loading = true;
  @state() private busy = false;
  @state() private entries: WardrobeEntryDto[] = [];
  @state() private activeId: string | null = null;
  @state() private selectedId: string | null = null;

  @state() private uploadName = "";
  @state() private uploadModel: "default" | "slim" = "default";

  @state() private query = "";
  @state() private preview: ResolvedSkinDto | null = null;
  @state() private previewErr = "";

  static styles = [
    controlStyles,
    pageChromeStyles,
    cardSurfaceStyles,
    css`
      :host {
        display: flex;
        flex-direction: column;
        flex: 1 0 auto;
        min-height: 100%;
        width: 100%;
        min-width: 0;
        box-sizing: border-box;
      }
      *,
      *::before,
      *::after {
        box-sizing: border-box;
      }
      /* h1 / .sub → pageChromeStyles (TYP-1); wardrobe keeps shorter sub measure */
      .sub {
        max-width: 42em;
        margin-bottom: var(--easymc-space-3);
      }
      .layout {
        display: grid;
        grid-template-columns: minmax(220px, 280px) minmax(0, 1fr);
        gap: var(--easymc-grid-gap);
        align-items: start;
      }
      @container easymc-page (max-width: 720px) {
        .layout {
          grid-template-columns: 1fr;
        }
      }
      .preview-card,
      .panel {
        padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
        border-radius: var(--md-sys-shape-corner-extra-large);
        background: var(--md-sys-color-surface-container-lowest);
        box-shadow: var(--md-sys-elevation-1); /* CF-1: elev XOR border */
        border: none;
        min-width: 0;
      }
      .back-row {
        margin-bottom: var(--easymc-space-2);
      }
      .preview-card {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: var(--easymc-space-3);
        position: sticky;
        top: var(--easymc-space-2);
      }
      /* Avoid nested elevation soup: 3D stage sits flat inside the card */
      .preview-card easymc-skin-viewer {
        box-shadow: none;
      }
      .preview-meta {
        width: 100%;
        text-align: center;
      }
      .preview-meta strong {
        display: block;
        font-size: var(--easymc-type-body);
        font-weight: var(--easymc-weight-medium);
        margin-bottom: 2px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 100%;
      }
      .preview-meta span {
        font-size: var(--easymc-type-meta);
        font-weight: var(--easymc-weight-regular);
        color: var(--md-sys-color-on-surface-variant);
      }
      .list {
        display: grid;
        gap: var(--easymc-space-2);
      }
      button.entry {
        display: flex;
        align-items: center;
        gap: var(--easymc-space-3);
        width: 100%;
        text-align: left;
        font: inherit;
        color: inherit;
        cursor: pointer;
        min-height: 64px;
        padding: var(--easymc-space-3) var(--easymc-space-4);
        border-radius: var(--md-sys-shape-corner-large);
        background: var(--md-sys-color-surface);
        border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
        box-shadow: none; /* CF-1: outline only */
        transition:
          border-color var(--easymc-dur-press) var(--easymc-ease-out),
          background var(--easymc-dur-press) var(--easymc-ease-out);
      }
      button.entry:focus-visible {
        outline: none;
        box-shadow: var(--easymc-focus-ring);
      }
      button.entry:hover {
        background: var(--md-sys-color-surface-container);
      }
      button.entry.on {
        border-color: var(--md-sys-color-primary);
        background: color-mix(in oklch, var(--md-sys-color-primary-container) 55%, var(--md-sys-color-surface));
      }
      button.entry .thumb {
        width: 40px;
        height: 40px;
        border-radius: var(--md-sys-shape-corner-medium);
        image-rendering: pixelated;
        background: var(--md-sys-color-surface-container);
        object-fit: cover;
      }
      button.entry .body {
        flex: 1;
        min-width: 0;
      }
      button.entry .name {
        font-weight: var(--easymc-weight-medium);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      button.entry .meta {
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
      }
      .badge {
        display: inline-flex;
        align-items: center;
        padding: 2px 8px;
        border-radius: var(--md-sys-shape-corner-full);
        font-size: var(--easymc-type-meta);
        font-weight: var(--easymc-weight-medium);
        background: var(--md-sys-color-secondary-container);
        color: var(--md-sys-color-on-secondary-container);
        flex-shrink: 0;
      }
      .row {
        display: flex;
        flex-wrap: wrap;
        gap: var(--easymc-space-2);
        align-items: center;
      }
      .form {
        display: grid;
        gap: var(--easymc-space-3);
        margin-top: var(--easymc-space-3);
      }
      label.field {
        display: grid;
        gap: var(--easymc-space-2);
        font-size: var(--easymc-type-label);
        color: var(--md-sys-color-on-surface-variant);
      }
      label.field input,
      label.field select {
        font: inherit;
        color: var(--md-sys-color-on-surface);
        background: var(--md-sys-color-surface);
        border: 1px solid var(--md-sys-color-outline-variant);
        border-radius: var(--md-sys-shape-corner-medium);
        padding: 10px 12px;
      }
      label.field input:focus-visible,
      label.field select:focus-visible {
        outline: none;
        box-shadow: var(--easymc-focus-ring);
      }
      .tip {
        font-size: var(--easymc-type-meta);
        font-weight: var(--easymc-weight-regular);
        color: var(--md-sys-color-on-surface-variant);
        margin: 0;
        line-height: 1.45;
        max-width: 36em;
      }
      .err {
        color: var(--md-sys-color-error);
        font-size: var(--easymc-type-meta);
        margin: 0;
      }
      .actions {
        display: flex;
        flex-wrap: wrap;
        gap: var(--easymc-space-2);
        margin-top: var(--easymc-space-3);
        justify-content: center;
        width: 100%;
      }
      .actions button.primary,
      .actions button.ghost,
      .actions button.danger,
      .actions button.tonal {
        padding: 7px 14px;
        font-size: var(--easymc-type-label);
      }
      button.entry .name {
        font-size: var(--easymc-type-body);
        font-weight: var(--easymc-weight-medium);
      }
      button.entry .meta {
        font-weight: var(--easymc-weight-regular);
      }
    `,
  ];

  connectedCallback() {
    super.connectedCallback();
    void this.refresh();
  }

  private get selected(): WardrobeEntryDto | null {
    return this.entries.find((e) => e.id === this.selectedId) ?? this.entries[0] ?? null;
  }

  private async refresh() {
    this.loading = true;
    try {
      const st = await wardrobeList();
      this.entries = st.entries;
      this.activeId = st.active_id;
      if (!this.selectedId || !this.entries.some((e) => e.id === this.selectedId)) {
        this.selectedId = st.active_id ?? this.entries[0]?.id ?? null;
      }
    } catch (e) {
      showToast(friendlyError(e), { ms: 3200 });
    } finally {
      this.loading = false;
    }
  }

  private async onUpload() {
    const path = await pickSkinPng("选择皮肤 PNG（64×64）");
    if (!path) return;
    let cape: string | null = null;
    if (confirm("要一起选一张披风 PNG 吗？（可跳过）")) {
      cape = await pickSkinPng("选择披风 PNG");
    }
    this.busy = true;
    try {
      const entry = await wardrobeUpload({
        skinPath: path,
        capePath: cape,
        name: this.uploadName.trim() || null,
        model: this.uploadModel,
      });
      this.emitWardrobeChanged();
      showToast(`已加入衣柜：${entry.name}`);
      await this.refresh();
      this.selectedId = entry.id;
      this.tab = "mine";
    } catch (e) {
      showToast(friendlyError(e), { ms: 3600 });
    } finally {
      this.busy = false;
    }
  }

  private async onReplaceSkin() {
    const cur = this.selected;
    if (!cur) return;
    const path = await pickSkinPng("替换皮肤 PNG");
    if (!path) return;
    this.busy = true;
    try {
      await wardrobeReplace({ id: cur.id, skinPath: path });
      showToast("皮肤已替换");
      await this.refresh();
    } catch (e) {
      showToast(friendlyError(e), { ms: 3600 });
    } finally {
      this.busy = false;
    }
  }


  private emitWardrobeChanged() {
    this.dispatchEvent(new CustomEvent("wardrobe-changed", { bubbles: true, composed: true }));
  }

  private async onSetActive() {
    const cur = this.selected;
    if (!cur) return;
    this.busy = true;
    try {
      const st = await wardrobeSetActive(cur.id);
      this.entries = st.entries;
      this.activeId = st.active_id;
      this.emitWardrobeChanged();
      showToast(`已选用「${cur.name}」`);
    } catch (e) {
      showToast(friendlyError(e), { ms: 3200 });
    } finally {
      this.busy = false;
    }
  }

  private async onRemove() {
    const cur = this.selected;
    if (!cur) return;
    if (!confirm(`从衣柜删除「${cur.name}」？`)) return;
    this.busy = true;
    try {
      const st = await wardrobeRemove(cur.id);
      this.entries = st.entries;
      this.activeId = st.active_id;
      this.selectedId = st.active_id ?? st.entries[0]?.id ?? null;
      this.emitWardrobeChanged();
      showToast("已删除");
    } catch (e) {
      showToast(friendlyError(e), { ms: 3200 });
    } finally {
      this.busy = false;
    }
  }

  private async onFetch(kind: "official" | "littleskin") {
    const q = this.query.trim();
    if (!q) {
      this.previewErr = kind === "official" ? "请输入正版玩家名" : "请输入 LittleSkin 玩家名";
      return;
    }
    this.busy = true;
    this.previewErr = "";
    this.preview = null;
    try {
      this.preview =
        kind === "official" ? await skinFetchOfficial(q) : await skinFetchLittleSkin(q);
    } catch (e) {
      this.previewErr = friendlyError(e);
    } finally {
      this.busy = false;
    }
  }

  private async onImport(kind: "official" | "littleskin") {
    const q = this.query.trim();
    if (!q) return;
    this.busy = true;
    try {
      const entry =
        kind === "official"
          ? await wardrobeImportOfficial(q)
          : await wardrobeImportLittleSkin(q);
      this.emitWardrobeChanged();
      showToast(`已导入：${entry.name}`);
      await this.refresh();
      this.selectedId = entry.id;
      this.tab = "mine";
      this.preview = null;
    } catch (e) {
      showToast(friendlyError(e), { ms: 3600 });
    } finally {
      this.busy = false;
    }
  }

  private renderPreviewPane() {
    const mine = this.selected;
    const remote = this.preview;
    const skin = remote?.skin_data_url || remote?.skin_url || mine?.skin_data_url || null;
    const cape = remote?.cape_data_url || remote?.cape_url || mine?.cape_data_url || null;
    const model = remote?.model || mine?.model || "auto-detect";
    const title = remote?.name || mine?.name || "还没有皮肤";
    const meta = remote
      ? `${remote.source_label}${remote.hasOwnProperty("cape_url") && remote.cape_url ? " · 有披风" : ""}`
      : mine
        ? `${mine.source_label}${mine.has_cape ? " · 有披风" : ""}${mine.id === this.activeId ? " · 当前选用" : ""}`
        : "上传或搜索一张试试";

    return html`
      <div class="preview-card">
        <easymc-skin-viewer
          style="--easymc-skin-viewer-w:240px;--easymc-skin-viewer-h:340px"
          .skin=${skin}
          .cape=${cape}
          .model=${model}
          .width=${240}
          .height=${340}
        ></easymc-skin-viewer>
        <div class="preview-meta">
          <strong>${title}</strong>
          <span>${meta}</span>
        </div>
        ${mine && !remote
          ? html`<div class="actions">
              <button class="primary" type="button" ?disabled=${this.busy || mine.id === this.activeId} @click=${() => void this.onSetActive()}>
                ${mine.id === this.activeId ? "已是当前" : "设为当前"}
              </button>
              <button class="ghost" type="button" ?disabled=${this.busy} @click=${() => void this.onReplaceSkin()}>替换</button>
              <button class="danger" type="button" ?disabled=${this.busy} @click=${() => void this.onRemove()}>删除</button>
            </div>`
          : nothing}
        ${remote
          ? html`<div class="actions">
              <button
                class="primary"
                type="button"
                ?disabled=${this.busy}
                @click=${() => void this.onImport(remote.source === "little_skin" ? "littleskin" : "official")}
              >
                导入衣柜
              </button>
            </div>`
          : nothing}
      </div>
    `;
  }

  private renderMine() {
    return html`
      <div class="panel">
        <p class="tip">本地衣柜保存在本机。选中后可三维预览（含披风）；设为当前后，启动游戏会套用这套皮肤。</p>
        <div class="form">
          <label class="field"
            >名称（可选）<input .value=${this.uploadName} @input=${(e: Event) => (this.uploadName = (e.target as HTMLInputElement).value)} placeholder="我的皮肤"
          /></label>
          <label class="field"
            >模型
            <select .value=${this.uploadModel} @change=${(e: Event) => (this.uploadModel = (e.target as HTMLSelectElement).value as "default" | "slim")}>
              <option value="default">经典（Steve）</option>
              <option value="slim">纤细（Alex）</option>
            </select>
          </label>
          <div class="row">
            <button class="primary" type="button" ?disabled=${this.busy} @click=${() => void this.onUpload()}>上传 PNG</button>
          </div>
        </div>
        <div class="list" style="margin-top:var(--easymc-space-4)" role="listbox" aria-label="衣柜列表">
          ${this.loading
            ? html`<div class="skel skel-block" style="height:64px;border-radius:var(--md-sys-shape-corner-large)"></div>`
            : this.entries.length === 0
              ? html`<easymc-empty-state variant="panel" title="衣柜还是空的" description="上传一张 PNG，或切到「正版 / LittleSkin」搜玩家名"></easymc-empty-state>`
              : this.entries.map((e) => {
                  const on = e.id === this.selectedId;
                  return html`<button
                    class="entry ${on ? "on" : ""}"
                    type="button"
                    role="option"
                    aria-selected=${on}
                    @click=${() => {
                      this.selectedId = e.id;
                      this.preview = null;
                    }}
                  >
                    <img class="thumb" src=${e.skin_data_url} alt="" width="40" height="40" />
                    <span class="body">
                      <div class="name">${e.name}</div>
                      <div class="meta">${e.source_label}${e.has_cape ? " · 披风" : ""}${e.id === this.activeId ? " · 当前" : ""}</div>
                    </span>
                    ${e.id === this.activeId ? html`<span class="badge">当前</span>` : nothing}
                  </button>`;
                })}
        </div>
      </div>
    `;
  }

  private renderSearch(kind: "official" | "littleskin") {
    const label = kind === "official" ? "正版玩家名" : "LittleSkin 玩家名";
    const tip =
      kind === "official"
        ? "从正版档案拉取皮肤与披风；国内会自动换更顺的线路。"
        : "按玩家名从 LittleSkin 取皮肤；国内访问更稳。";
    return html`
      <div class="panel">
        <p class="tip">${tip}</p>
        <div class="form">
          <label class="field"
            >${label}<input
              .value=${this.query}
              @input=${(e: Event) => (this.query = (e.target as HTMLInputElement).value)}
              @keydown=${(e: KeyboardEvent) => {
                if (e.key === "Enter") void this.onFetch(kind);
              }}
              placeholder=${kind === "official" ? "Steve 或 UUID" : "玩家名"}
            />
          </label>
          <div class="row">
            <button class="primary" type="button" ?disabled=${this.busy} @click=${() => void this.onFetch(kind)}>预览</button>
            <button class="tonal" type="button" ?disabled=${this.busy || !this.preview} @click=${() => void this.onImport(kind)}>导入衣柜</button>
          </div>
          ${this.previewErr ? html`<p class="err" role="alert">${this.previewErr}</p>` : nothing}
        </div>
      </div>
    `;
  }

  render() {
    return html`
      ${this.fromAccounts
        ? html`<div class="row back-row">
            <button
              class="ghost"
              type="button"
              @click=${() =>
                this.dispatchEvent(
                  new CustomEvent("nav-change", { detail: "accounts", bubbles: true, composed: true }),
                )}
            >
              ← 账户
            </button>
          </div>`
        : nothing}
      <h1>皮肤衣柜</h1>
      <p class="sub">三维预览（含披风）、本地上传，以及正版 / LittleSkin 导入。</p>
      <div class="md-tabs" role="tablist" aria-label="衣柜分区">
        <button type="button" class=${this.tab === "mine" ? "on" : ""} @click=${() => (this.tab = "mine")}>我的</button>
        <button type="button" class=${this.tab === "official" ? "on" : ""} @click=${() => (this.tab = "official")}>正版</button>
        <button type="button" class=${this.tab === "littleskin" ? "on" : ""} @click=${() => (this.tab = "littleskin")}>LittleSkin</button>
      </div>
      <div class="layout">
        ${this.renderPreviewPane()}
        ${this.tab === "mine"
          ? this.renderMine()
          : this.tab === "official"
            ? this.renderSearch("official")
            : this.renderSearch("littleskin")}
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-wardrobe-page": WardrobePage;
  }
}
