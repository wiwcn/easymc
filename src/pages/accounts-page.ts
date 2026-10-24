import { LitElement, css, html, nothing } from "lit";
import { controlStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  createOfflineOrThrow,
  listAccountsState,
  removeAccount,
  setActiveAccount,
} from "../api";
import type { AccountDto } from "../types";
import { showToast } from "../toast";
import "../components/empty-state";
import "../components/msa-login";

/**
 * Standalone accounts page (PCL-aligned): list + add flow.
 * Not nested under Settings.
 */
@customElement("easymc-accounts-page")
export class AccountsPage extends LitElement {
  @state() private accounts: AccountDto[] = [];
  @state() private activeAccountId: string | null = null;
  @state() private loading = true;
  /** null = list; chooser = add type; offline/microsoft/skin = forms */
  @state() private panel: null | "chooser" | "offline" | "microsoft" | "skin" = null;
  @state() private name = "";
  @state() private nameError = "";
  @state() private skinServer = "";
  @state() private skinUser = "";
  @state() private skinNote = "";

  static styles = [
    controlStyles,
    css`
      :host {
        display: flex;
        flex-direction: column;
        flex: 1 0 auto;
        min-height: 100%;
        max-width: 720px;
        width: 100%;
      }
      h1 {
        font-size: var(--easymc-type-title);
        font-weight: var(--easymc-weight-bold);
        margin: 0 0 2px;
        letter-spacing: -0.02em;
      }
      .sub {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-meta);
        margin: 0 0 var(--easymc-space-4);
      }
      .surf {
        flex: 1 1 auto;
        display: flex;
        flex-direction: column;
        min-height: 360px;
        padding: var(--easymc-space-4);
        border-radius: var(--md-sys-shape-corner-large);
        background: var(--md-sys-color-surface-container-lowest);
        box-shadow: var(--md-sys-elevation-1); /* CF-1: elevation, no border */
      }
      .acct-list {
        display: grid;
        gap: var(--easymc-space-2);
      }
      .acct {
        display: flex;
        align-items: center;
        gap: var(--easymc-space-3);
        width: 100%;
        text-align: left;
        font: inherit;
        color: inherit;
        cursor: pointer;
        padding: 12px 14px;
        border-radius: 16px;
        background: var(--md-sys-color-surface);
        border: 1.5px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 45%, transparent);
        transition:
          border-color var(--easymc-dur-press) var(--easymc-ease-out),
          background var(--easymc-dur-press) var(--easymc-ease-out);
      }
      .acct:hover {
        background: var(--md-sys-color-surface-container);
      }
      .acct.active {
        border-color: var(--md-sys-color-primary);
        background: var(--md-sys-color-primary-container);
        color: var(--md-sys-color-on-primary-container);
      }
      .skin-ph {
        width: 44px;
        height: 44px;
        border-radius: 10px;
        flex-shrink: 0;
        background: linear-gradient(
          180deg,
          oklch(0.78 0.06 70) 0 38%,
          oklch(0.55 0.1 220) 38% 62%,
          oklch(0.45 0.08 250) 62% 100%
        );
        box-shadow: var(--md-sys-elevation-1);
        position: relative;
        overflow: hidden;
      }
      .skin-ph::after {
        content: "";
        position: absolute;
        inset: 8px 14px 10px;
        border-radius: 4px;
        background: oklch(0.92 0.03 90 / 0.35);
      }
      .skin-ph.ms {
        background: linear-gradient(135deg, oklch(0.55 0.12 250), oklch(0.45 0.1 220));
      }
      .skin-ph.skin {
        background: linear-gradient(135deg, oklch(0.62 0.14 330), oklch(0.5 0.12 300));
      }
      .acct-body {
        flex: 1;
        min-width: 0;
      }
      .acct-name {
        font-weight: var(--easymc-weight-medium);
        font-size: var(--easymc-type-body);
        letter-spacing: -0.01em;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .acct-meta {
        font-size: var(--easymc-type-meta);
        opacity: 0.85;
        margin-top: 2px;
        display: flex;
        gap: 6px;
        flex-wrap: wrap;
        align-items: center;
      }
      .badge {
        font-size: 0.68rem;
        font-weight: 700;
        padding: 2px 8px;
        border-radius: 999px;
        background: var(--md-sys-color-secondary-container);
        color: var(--md-sys-color-on-secondary-container);
      }
      .badge.ms {
        background: oklch(0.55 0.12 250 / 0.2);
        color: oklch(0.4 0.1 250);
      }
      .badge.off {
        background: oklch(0.7 0.05 150 / 0.25);
        color: oklch(0.4 0.08 150);
      }
      .badge.skin {
        background: oklch(0.65 0.12 330 / 0.22);
        color: oklch(0.42 0.12 330);
      }
      .acct-actions {
        display: flex;
        gap: 4px;
        flex-shrink: 0;
      }
      .acct-actions button {
        border: none;
        background: transparent;
        cursor: pointer;
        padding: 6px 10px;
        border-radius: 999px;
        font-size: var(--easymc-type-meta);
        font-weight: var(--easymc-weight-medium);
        color: var(--md-sys-color-on-surface-variant);
      }
      .acct-actions button:hover {
        background: var(--md-sys-color-surface-container-highest);
      }
      .acct-actions button.danger:hover {
        color: var(--md-sys-color-error);
      }
      .chooser {
        display: grid;
        gap: 10px;
        margin-top: 8px;
      }
      button.choice {
        text-align: left;
        font: inherit;
        color: inherit;
        cursor: pointer;
        padding: 14px 16px;
        border-radius: 16px;
        background: var(--md-sys-color-surface);
        border: 1.5px solid var(--md-sys-color-outline-variant);
      }
      button.choice:hover {
        border-color: var(--md-sys-color-primary);
        background: var(--md-sys-color-surface-container);
      }
      button.choice strong {
        display: block;
        font-size: var(--easymc-type-body);
        margin-bottom: 4px;
      }
      button.choice span {
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
        line-height: 1.4;
      }
      .form-card {
        margin-top: 10px;
        padding: 14px;
        border-radius: 16px;
        background: var(--md-sys-color-surface-container-low);
      }
      .form-card label {
        display: flex;
        flex-direction: column;
        gap: 6px;
        margin-bottom: 10px;
        font-size: var(--easymc-type-label);
        color: var(--md-sys-color-on-surface-variant);
      }
      .form-card input {
        border: 1px solid var(--md-sys-color-outline-variant);
        border-radius: 12px;
        padding: 8px 12px;
        background: var(--md-sys-color-surface-container-lowest);
        color: var(--md-sys-color-on-surface);
        font: inherit;
      }
      .err {
        color: var(--md-sys-color-error);
        font-size: var(--easymc-type-label);
        margin: -4px 0 8px;
      }
      .tip {
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
        line-height: 1.45;
        margin-bottom: 8px;
      }
      .row {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        align-items: center;
      }
      .acct-empty-slot {
        flex: 1 1 auto;
        display: flex;
        flex-direction: column;
        min-height: 240px;
        align-self: stretch;
      }
      .acct-empty-slot easymc-empty-state {
        flex: 1 1 auto;
        min-height: 240px;
        width: 100%;
      }
      .foot {
        margin-top: var(--easymc-space-4);
      }
    `,
  ];

  connectedCallback() {
    super.connectedCallback();
    void this.refresh();
  }

  private async refresh() {
    this.loading = true;
    try {
      const a = await listAccountsState();
      this.accounts = a.accounts;
      this.activeAccountId = a.active_id;
    } finally {
      this.loading = false;
    }
  }

  private kindLabel(kind: string): string {
    if (kind === "microsoft") return "正版";
    if (kind === "skin_station") return "皮肤站";
    return "离线";
  }

  private kindBadgeClass(kind: string): string {
    if (kind === "microsoft") return "ms";
    if (kind === "skin_station") return "skin";
    return "off";
  }

  private validateName(raw: string): string {
    const n = raw.trim();
    const len = [...n].length;
    if (!n) return "请输入游戏里显示的名字";
    if (len < 2) return "名字至少 2 个字符";
    if (len > 16) return "名字最多 16 个字符";
    return "";
  }

  render() {
    return html`
      <h1>账户</h1>
      <p class="sub">管理登录方式与当前使用的名字。点选切换；正版服请用微软账户。</p>
      <div class="surf" ?aria-busy=${this.loading}>
        ${this.loading
          ? html`<div class="acct-list" aria-label="正在加载账户">
              ${[1, 2, 3].map(() => html`<div class="skel skel-block" style="height:68px;border-radius:16px"></div>`)}
            </div>`
          : this.panel === null
            ? this.renderList()
            : this.panel === "chooser"
              ? this.renderChooser()
              : this.panel === "offline"
                ? this.renderOffline()
                : this.panel === "microsoft"
                  ? this.renderMs()
                  : this.renderSkin()}
      </div>
    `;
  }

  private renderList() {
    return html`
      <div class="acct-list" role="listbox" aria-label="账户列表">
        ${this.accounts.map((a) => {
          const active = a.id === this.activeAccountId;
          return html`<div
            class="acct ${active ? "active" : ""}"
            role="option"
            tabindex="0"
            aria-selected=${active}
            @click=${async () => {
              const st = await setActiveAccount(a.id);
              this.accounts = st.accounts;
              this.activeAccountId = st.active_id;
              showToast(`已切换到 ${a.username}`);
            }}
            @keydown=${(e: KeyboardEvent) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                (e.currentTarget as HTMLElement).click();
              }
            }}
          >
            <span class="skin-ph ${this.kindBadgeClass(a.kind)}" aria-hidden="true"></span>
            <span class="acct-body">
              <div class="acct-name">${a.username}</div>
              <div class="acct-meta">
                <span class="badge ${this.kindBadgeClass(a.kind)}">${this.kindLabel(a.kind)}</span>
                ${active ? html`<span>当前使用</span>` : nothing}
              </div>
            </span>
            <span class="acct-actions" @click=${(e: Event) => e.stopPropagation()}>
              <button
                class="text"
                type="button"
                title="移除"
                style="color:var(--md-sys-color-error)"
                @click=${async () => {
                  if (!confirm(`移除账户「${a.username}」？`)) return;
                  const st = await removeAccount(a.id);
                  this.accounts = st.accounts;
                  this.activeAccountId = st.active_id;
                  showToast("已移除");
                }}
              >
                移除
              </button>
            </span>
          </div>`;
        })}
      </div>
      ${!this.accounts.length
        ? html`<div class="acct-empty-slot">
            <easymc-empty-state
              variant="panel"
              title="咦？还没有账户呢…"
              description="点下面加一个，就能开玩啦"
            >
              <button class="primary" type="button" @click=${() => (this.panel = "chooser")}>
                添加账户
              </button>
            </easymc-empty-state>
          </div>`
        : html`<div class="row foot">
            <button class="primary" type="button" @click=${() => (this.panel = "chooser")}>
              添加账户
            </button>
          </div>`}
    `;
  }

  private renderChooser() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.panel = null)}>← 返回列表</button>
      </div>
      <p class="tip" style="margin-top:0">选择登录方式</p>
      <div class="chooser">
        <button class="choice" type="button" @click=${() => (this.panel = "microsoft")}>
          <strong>正版微软账户</strong>
          <span>使用微软账户登录，可进入正版服务器。</span>
        </button>
        <button
          class="choice"
          type="button"
          @click=${() => {
            this.name = "";
            this.nameError = "";
            this.panel = "offline";
          }}
        >
          <strong>离线游玩</strong>
          <span>输入游戏里显示的名字，马上就能玩单人或离线服。</span>
        </button>
        <button class="choice" type="button" @click=${() => (this.panel = "skin")}>
          <strong>第三方皮肤站</strong>
          <span>Authlib / 统一通行证等皮肤站账号（即将支持完整登录）。</span>
        </button>
      </div>
    `;
  }

  private renderOffline() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.panel = "chooser")}>← 返回</button>
      </div>
      <div class="form-card">
        <div style="display:flex;justify-content:center;margin-bottom:12px">
          <span
            class="skin-ph off"
            style="width:64px;height:64px;border-radius:12px"
            aria-hidden="true"
          ></span>
        </div>
        <label
          >游戏用户名
          <input
            maxlength="16"
            placeholder="例如：Steve"
            .value=${this.name}
            @input=${(e: Event) => {
              this.name = (e.target as HTMLInputElement).value;
              this.nameError = "";
            }}
            @keydown=${(e: KeyboardEvent) => {
              if (e.key === "Enter") void this.submitOffline();
            }}
          />
        </label>
        ${this.nameError
          ? html`<div class="err" role="alert">${this.nameError}</div>`
          : html`<div class="tip">
              2–16 个字符。离线可用中文名；进多数服务器建议只用英文字母、数字和下划线。
            </div>`}
        <div class="row">
          <button class="primary" type="button" @click=${() => this.submitOffline()}>
            添加并使用
          </button>
        </div>
      </div>
    `;
  }

  private async submitOffline() {
    const err = this.validateName(this.name);
    this.nameError = err;
    if (err) return;
    try {
      const acc = await createOfflineOrThrow(this.name.trim());
      showToast(`已切换到 ${acc.username}`);
      this.panel = null;
      await this.refresh();
    } catch (e) {
      this.nameError = e instanceof Error ? e.message : String(e);
    }
  }

  private renderMs() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.panel = "chooser")}>← 返回</button>
      </div>
      <div class="form-card">
        <p class="tip" style="margin-top:0">用浏览器登录微软账户，EasyMC 不会看到你的密码。</p>
        <easymc-msa-login
          @msa-done=${async (e: CustomEvent) => {
            showToast(`已登录 ${e.detail.username}`);
            this.panel = null;
            await this.refresh();
          }}
        ></easymc-msa-login>
      </div>
    `;
  }

  private renderSkin() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.panel = "chooser")}>← 返回</button>
      </div>
      <div class="form-card">
        <p class="tip" style="margin-top:0">皮肤站登录即将支持。可先填信息并起一个离线名字进入游戏。</p>
        <label
          >皮肤站地址<input
            .value=${this.skinServer}
            placeholder="例如：https://littleskin.cn"
            @input=${(e: Event) => (this.skinServer = (e.target as HTMLInputElement).value)}
        /></label>
        <label
          >账号（邮箱或用户名）<input
            .value=${this.skinUser}
            @input=${(e: Event) => (this.skinUser = (e.target as HTMLInputElement).value)}
        /></label>
        <label
          >游戏里显示的名字<input
            maxlength="16"
            .value=${this.name}
            @input=${(e: Event) => {
              this.name = (e.target as HTMLInputElement).value;
              this.nameError = "";
            }}
        /></label>
        ${this.nameError ? html`<div class="err">${this.nameError}</div>` : nothing}
        ${this.skinNote ? html`<div class="tip">${this.skinNote}</div>` : nothing}
        <div class="row">
          <button
            class="ghost"
            type="button"
            @click=${() => {
              this.skinNote = "已记下，皮肤站登录上线后会提醒你完成。";
              try {
                localStorage.setItem(
                  "easymc.skinStationDraft",
                  JSON.stringify({ server: this.skinServer, user: this.skinUser }),
                );
              } catch {
                /* */
              }
            }}
          >
            稍后再说
          </button>
          <button class="primary" type="button" @click=${() => this.submitOffline()}>
            添加离线名并使用
          </button>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-accounts-page": AccountsPage;
  }
}
