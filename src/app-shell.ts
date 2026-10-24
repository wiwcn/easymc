import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import { keyed } from "lit/directives/keyed.js";
import type { AccountDto, AppMode, NavId } from "./types";
import { getSettings, listAccountsState } from "./api";
import "./pages/onboarding-page";
import "./components/side-nav";
import "./components/skin-avatar";
import "./pages/home-page";
import "./pages/download-page";
import "./pages/instance-page";
import "./pages/multiplayer-page";
import "./pages/accounts-page";
import "./pages/settings-page";
import "./pages/more-page";
import "./pages/agent-page";

@customElement("easymc-app")
export class EasyMcApp extends LitElement {
  @state() private route: NavId = "home";
  @state() private pageKey = 0;
  @state() private ready = false;
  @state() private onboarded = true;
  @state() private mode: AppMode = "standard";
  @state() private activeAccount: AccountDto | null = null;

  static styles = css`
    :host {
      display: flex;
      height: 100vh;
      width: 100vw;
      background: var(--md-sys-color-surface);
    }
    .main {
      flex: 1;
      display: flex;
      flex-direction: column;
      min-width: 0;
      overflow: hidden;
    }
    header.top {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: var(--easymc-space-2) var(--easymc-space-5);
      min-height: 52px;
      background: var(--md-sys-color-surface);
      border-bottom: 1px solid
        color-mix(in oklch, var(--md-sys-color-outline-variant) 50%, transparent);
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
      font-weight: 650;
      font-size: var(--easymc-type-title);
      letter-spacing: -0.025em;
      color: var(--md-sys-color-on-surface);
    }
    /* A11Y-1: topbar menu removed until side-nav collapse ships */
    .account {
      display: flex;
      align-items: center;
      gap: 6px;
      border: none;
      background: transparent;
      cursor: pointer;
      padding: 4px 6px 4px 4px;
      border-radius: var(--md-sys-shape-corner-full);
    }
    .account:hover {
      background: var(--md-sys-color-surface-container);
    }
    .account easymc-skin-avatar {
      --easymc-skin-avatar-size: 34px;
      border-radius: 50%;
      border: 2px solid var(--md-sys-color-outline-variant);
    }
    .chev {
      color: var(--md-sys-color-on-surface-variant);
      display: grid;
      place-items: center;
    }
    .content {
      flex: 1;
      min-height: 0;
      overflow: auto;
      padding: var(--easymc-page-pad-y) var(--easymc-page-pad-x) var(--easymc-space-6);
      background: var(--md-sys-color-surface);
      display: flex;
      flex-direction: column;
    }
    .page {
      flex: 1 1 auto;
      min-height: 100%;
      display: flex;
      flex-direction: column;
      animation: page-in var(--easymc-dur-page) var(--easymc-ease-out) both;
    }
    .page > * {
      flex: 1 0 auto;
      min-height: 100%;
      box-sizing: border-box;
      width: 100%;
    }
    /* MD3 top-level transition: fade-through, NO spatial displacement (C-3).
       Displacement is reserved for hierarchical (list → detail) navigation. */
    @keyframes page-in {
      from {
        opacity: 0;
      }
      to {
        opacity: 1;
      }
    }
    @media (prefers-reduced-motion: reduce) {
      .page {
        animation: none;
      }
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    this.addEventListener("nav-change", this.onNav as EventListener);
    this.addEventListener("onboarding-done", this.onOnboarded);
    this.addEventListener("mode-change", this.onModeChange as EventListener);
    void this.loadSettings();
    void this.loadAccounts();
  }

  private async loadSettings() {
    const s = await getSettings();
    this.onboarded = !!s.onboarding_complete;
    this.mode = s.mode === "agent" ? "agent" : "standard";
    const navParam = new URLSearchParams(location.search).get("nav");
    const navAlias: Record<string, NavId> = { instance: "instances", library: "instances", store: "download" };
    const nav = (navAlias[navParam ?? ""] ?? navParam) as NavId | null;
    if (nav && ["home", "download", "instances", "multiplayer", "accounts", "settings", "more", "agent"].includes(nav)) {
      this.route = nav;
    }
    this.ready = true;
  }

  private async loadAccounts() {
    try {
      const st = await listAccountsState();
      const id = st.active_id;
      this.activeAccount = (id && st.accounts.find((a) => a.id === id)) || st.accounts[0] || null;
    } catch {
      this.activeAccount = null;
    }
  }

  private onOnboarded = () => {
    void this.loadSettings().then(() => {
      this.route = "home";
      this.pageKey += 1;
      void this.loadAccounts();
    });
  };

  private onModeChange = (e: Event) => {
    const m = (e as CustomEvent<AppMode>).detail;
    if (m) this.mode = m;
  };

  disconnectedCallback() {
    this.removeEventListener("nav-change", this.onNav as EventListener);
    this.removeEventListener("onboarding-done", this.onOnboarded);
    this.removeEventListener("mode-change", this.onModeChange as EventListener);
    super.disconnectedCallback();
  }

  private onNav = (e: Event) => {
    const detail = (e as CustomEvent<NavId>).detail;
    if (!detail || detail === this.route) return;
    const map: Record<string, NavId> = {
      library: "instances",
      store: "download",
      share: "more",
    };
    this.route = (map[detail] as NavId) || detail;
    this.pageKey += 1;
    // Refresh active account when leaving accounts (user may have switched).
    void this.loadAccounts();
  };

  private goAccounts() {
    this.route = "accounts";
    this.pageKey += 1;
    void this.loadAccounts();
  }

  render() {
    if (!this.ready) return html``;
    if (!this.onboarded) return html`<easymc-onboarding></easymc-onboarding>`;
    return html`
      <easymc-side-nav
        .active=${this.route}
        .mode=${this.mode}
        .account=${this.activeAccount}
      ></easymc-side-nav>
      <div class="main">
        <header class="top">
          <div class="brand">
            <span>EasyMC</span>
          </div>
          <button class="account" type="button" title="账户" @click=${() => this.goAccounts()}>
            <easymc-skin-avatar .account=${this.activeAccount} .size=${64}></easymc-skin-avatar>
            <span class="chev" aria-hidden="true">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M6 9l6 6 6-6" />
              </svg>
            </span>
          </button>
        </header>
        <div class="content">
          ${keyed(
            this.pageKey,
            html`<div class="page">${this.renderPage()}</div>`,
          )}
        </div>
      </div>
    `;
  }

  private renderPage() {
    switch (this.route) {
      case "home":
        return html`<easymc-home-page></easymc-home-page>`;
      case "download":
        return html`<easymc-download-page></easymc-download-page>`;
      case "instances":
        return html`<easymc-instance-page></easymc-instance-page>`;
      case "multiplayer":
        return html`<easymc-multiplayer-page></easymc-multiplayer-page>`;
      case "accounts":
        return html`<easymc-accounts-page></easymc-accounts-page>`;
      case "settings":
        return html`<easymc-settings-page></easymc-settings-page>`;
      case "more":
        return html`<easymc-more-page .mode=${this.mode}></easymc-more-page>`;
      case "agent":
        return html`<easymc-agent-page></easymc-agent-page>`;
      default:
        return html`<easymc-home-page></easymc-home-page>`;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-app": EasyMcApp;
  }
}
