import { LitElement, css, html, nothing, svg } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { AccountDto, AppMode, NavId } from "../types";
import { accountKindBadgeClass, accountKindLabel } from "../lib/skin-avatar";
import { syncIndicatorWithin } from "../lib/indicator";
import "./skin-avatar";

type Item = { id: NavId; label: string; icon: string };

/** Filled icons when active; outlined when idle — MD3 sketch richness. */
const ICONS_OUTLINE: Record<string, unknown> = {
  home: svg`<path d="M4 10.5 12 4l8 6.5V20a1 1 0 0 1-1 1h-5v-6H10v6H5a1 1 0 0 1-1-1v-9.5Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>`,
  download: svg`<path d="M12 3v10m0 0 4-4m-4 4-4-4M5 17v2a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-2" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>`,
  multiplayer: svg`<path d="M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm8 0a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM4.5 19c.6-2.2 2.6-3.5 5.5-3.5s4.9 1.3 5.5 3.5M14 15.5c1 .3 2.1.5 3.5.5 2.4 0 4-.8 4.5 2" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/>`,
  settings: svg`<path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" stroke="currentColor" stroke-width="1.8" fill="none"/><path d="M19.4 13a7.8 7.8 0 0 0 .1-2l1.7-1.3-1.7-3-2 .5a7.6 7.6 0 0 0-1.7-1L15.4 3h-3.8l-.4 2.2a7.6 7.6 0 0 0-1.7 1l-2-.5-1.7 3L7.5 11a7.8 7.8 0 0 0 0 2l-1.7 1.3 1.7 3 2-.5c.5.4 1.1.8 1.7 1l.4 2.2h3.8l.4-2.2c.6-.2 1.2-.6 1.7-1l2 .5 1.7-3L19.4 13Z" stroke="currentColor" stroke-width="1.4" fill="none"/>`,
  more: svg`<circle cx="6" cy="12" r="1.6" fill="currentColor"/><circle cx="12" cy="12" r="1.6" fill="currentColor"/><circle cx="18" cy="12" r="1.6" fill="currentColor"/>`,
  agent: svg`<path d="M8 10h8M9 14h6M7 7h10a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H13l-3 3v-3H7a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>`,
};

const ICONS_FILLED: Record<string, unknown> = {
  home: svg`<path d="M4 10.5 12 4l8 6.5V20a1 1 0 0 1-1 1h-5v-6H10v6H5a1 1 0 0 1-1-1v-9.5Z" fill="currentColor"/>`,
  download: svg`<path d="M11 3h2v9.2l2.6-2.6 1.4 1.4L12 16l-5-5 1.4-1.4L11 12.2V3Z" fill="currentColor"/><path d="M5 17h14v2a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-2Z" fill="currentColor"/>`,
  multiplayer: svg`<circle cx="8" cy="8" r="3" fill="currentColor"/><circle cx="16" cy="8" r="3" fill="currentColor"/><path d="M2.5 19c.7-2.6 3-4 5.5-4s4.8 1.4 5.5 4H2.5Zm9.5 0c.4-1.5 1.4-2.6 2.8-3.2 1 .4 2.1.7 3.7.7 2.2 0 3.7.8 4.3 2.5H12Z" fill="currentColor"/>`,
  settings: svg`<path d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Zm7.4 4.2a7.8 7.8 0 0 0 .1-1.4l1.8-1.4-1.8-3.1-2.1.5a7.6 7.6 0 0 0-1.8-1L14.9 2h-3.8l-.5 2.3a7.6 7.6 0 0 0-1.8 1l-2.1-.5-1.8 3.1 1.8 1.4a7.8 7.8 0 0 0 0 1.4l-1.8 1.4 1.8 3.1 2.1-.5c.5.4 1.1.8 1.8 1l.5 2.3h3.8l.5-2.3c.6-.2 1.3-.6 1.8-1l2.1.5 1.8-3.1-1.8-1.4Z" fill="currentColor"/>`,
  more: svg`<circle cx="6" cy="12" r="2" fill="currentColor"/><circle cx="12" cy="12" r="2" fill="currentColor"/><circle cx="18" cy="12" r="2" fill="currentColor"/>`,
  agent: svg`<path d="M7 6h10a3 3 0 0 1 3 3v7a3 3 0 0 1-3 3h-3.2L10 22v-3H7a3 3 0 0 1-3-3V9a3 3 0 0 1 3-3Zm1 5h8v2H8v-2Zm1 4h6v2H9v-2Z" fill="currentColor"/>`,
};

const NAV: Item[] = [
  { id: "home", label: "启动", icon: "home" },
  { id: "download", label: "下载", icon: "download" },
  { id: "agent", label: "助手", icon: "agent" },
  { id: "multiplayer", label: "联机", icon: "multiplayer" },
  // Accounts: bottom acct-card only (no duplicate nav item).
  { id: "settings", label: "设置", icon: "settings" },
  { id: "more", label: "更多", icon: "more" },
];

@customElement("easymc-side-nav")
export class SideNav extends LitElement {
  @property({ type: String }) active: NavId = "home";
  @property({ type: String }) mode: AppMode = "standard";
  /** Active account from shell (`list_accounts` / active). Null = empty state. */
  @property({ attribute: false }) account: AccountDto | null = null;

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      width: var(--easymc-nav-width);
      padding: 18px 12px 14px;
      background: var(--md-sys-color-surface-container-lowest);
      border-right: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 70%, transparent);
      flex-shrink: 0;
      gap: var(--easymc-space-2);
    }
    nav {
      position: relative;
      isolation: isolate;
      display: flex;
      flex-direction: column;
      gap: 2px;
      flex: 1;
    }
    /* MI-1: active pill morphs between items instead of per-button fill. */
    .nav-ind {
      position: absolute;
      left: 0;
      top: 0;
      z-index: -1;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary-container);
      opacity: 0;
      pointer-events: none;
      transition:
        transform var(--easymc-dur-med) var(--easymc-ease-out),
        width var(--easymc-dur-med) var(--easymc-ease-out),
        height var(--easymc-dur-med) var(--easymc-ease-out),
        opacity var(--easymc-dur-fast) var(--easymc-ease-out);
    }
    button.item {
      display: flex;
      align-items: center;
      gap: 12px;
      border: none;
      background: transparent;
      color: var(--md-sys-color-on-surface-variant);
      padding: 11px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      cursor: pointer;
      font-size: var(--easymc-type-body);
      font-weight: 500;
      text-align: left;
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.item:not(.active):hover {
      background: var(--md-sys-color-surface-container);
    }
    button.item:active {
      transform: scale(0.98);
    }
    /* Pill comes from .nav-ind so it can morph; active only recolours the label. */
    button.item.active {
      background: transparent;
      color: var(--md-sys-color-on-primary-container);
      font-weight: 600;
    }
    button.item:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }
    .ico {
      width: 22px;
      height: 22px;
      flex-shrink: 0;
      display: grid;
      place-items: center;
    }
    .ico svg {
      width: 20px;
      height: 20px;
      display: block;
    }
    .pill {
      margin-left: auto;
      font-size: var(--easymc-type-meta);
      padding: 2px 8px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      font-weight: 600;
    }
    button.acct-card {
      margin-top: auto;
      display: flex;
      gap: 10px;
      align-items: center;
      width: 100%;
      padding: 12px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      text-align: left;
      font: inherit;
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.acct-card:hover {
      background: var(--md-sys-color-surface-container);
    }
    button.acct-card:active {
      transform: scale(0.98);
    }
    button.acct-card:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }
    /* MD3 state layer (§4.2): 8% hover / 12% focus · pressed, below the label */
    button.item,
    button.acct-card {
      position: relative;
      isolation: isolate;
    }
    button.item {
      overflow: hidden; /* clip ripple (§4.7) */
    }
    button.item::after,
    button.acct-card::after {
      content: "";
      position: absolute;
      inset: 0;
      z-index: -1;
      border-radius: inherit;
      background: currentColor;
      opacity: 0;
      pointer-events: none;
      transition: opacity var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.item:hover::after,
    button.acct-card:hover::after {
      opacity: var(--md-sys-state-hover-opacity);
    }
    button.item:focus-visible::after,
    button.acct-card:focus-visible::after {
      opacity: var(--md-sys-state-focus-opacity);
    }
    button.item:active::after,
    button.acct-card:active::after {
      opacity: var(--md-sys-state-pressed-opacity);
    }
    button.acct-card easymc-skin-avatar {
      --easymc-skin-avatar-size: 36px;
    }
    .acct-body {
      flex: 1;
      min-width: 0;
    }
    .acct-name {
      display: block;
      font-size: var(--easymc-type-label);
      font-weight: 650;
      letter-spacing: -0.01em;
      margin-bottom: 3px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .acct-meta {
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.35;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      padding: 1px 7px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 11px;
      font-weight: 600;
      letter-spacing: 0.01em;
    }
    .badge.ms {
      background: var(--easymc-acct-ms-bg);
      color: var(--easymc-acct-ms-fg);
    }
    .badge.off {
      background: var(--easymc-acct-off-bg);
      color: var(--easymc-acct-off-fg);
    }
    .badge.skin {
      background: var(--easymc-acct-skin-bg);
      color: var(--easymc-acct-skin-fg);
    }
    .acct-chev {
      color: var(--md-sys-color-on-surface-variant);
      opacity: 0.7;
      flex-shrink: 0;
    }
    @media (prefers-reduced-motion: reduce) {
      button.item:active,
      button.acct-card:active {
        transform: none;
      }
      .nav-ind {
        transition: none;
      }
    }
  `;

  render() {
    const acct = this.account;
    const empty = !acct;

    return html`
      <nav aria-label="主导航">
        <span class="nav-ind" aria-hidden="true"></span>
        ${NAV.map((it) => {
          const on = this.active === it.id;
          return html`
            <button
              class="item ${on ? "active" : ""}"
              type="button"
              aria-current=${on ? "page" : "false"}
              @click=${() => this.select(it.id)}
            >
              <span class="ico" aria-hidden="true"
                ><svg viewBox="0 0 24 24">
                  ${on ? ICONS_FILLED[it.icon] : ICONS_OUTLINE[it.icon]}
                </svg></span
              >
              <span>${it.label}</span>
              ${it.id === "agent" && this.mode === "agent"
                ? html`<span class="pill">已开启</span>`
                : nothing}
            </button>
          `;
        })}
      </nav>
      <button
        class="acct-card"
        type="button"
        title=${empty ? "添加账户" : "账户管理"}
        @click=${() => this.select("accounts")}
      >
        <easymc-skin-avatar .account=${acct} .size=${64}></easymc-skin-avatar>
        <span class="acct-body">
          ${empty
            ? html`
                <span class="acct-name">添加账户</span>
                <span class="acct-meta">登录后可开玩</span>
              `
            : html`
                <span class="acct-name">${acct.username}</span>
                <span class="acct-meta">
                  <span class="badge ${accountKindBadgeClass(acct.kind)}"
                    >${accountKindLabel(acct.kind)}</span
                  >
                </span>
              `}
        </span>
        <span class="acct-chev" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M9 6l6 6-6 6" />
          </svg>
        </span>
      </button>
    `;
  }

  protected updated() {
    const nav = this.renderRoot.querySelector("nav");
    if (nav) syncIndicatorWithin(nav, ".nav-ind", "button.active");
  }

  private select(id: NavId) {
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: id, bubbles: true, composed: true }),
    );
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-side-nav": SideNav;
  }
}
