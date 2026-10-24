import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import { keyed } from "lit/directives/keyed.js";
import type { AppMode, NavId } from "./types";
import { getSettings } from "./api";
import "./pages/onboarding-page";
import "./components/side-nav";
import "./pages/home-page";
import "./pages/download-page";
import "./pages/instance-page";
import "./pages/multiplayer-page";
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
      padding: 10px 22px;
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
      font-size: 1.15rem;
      letter-spacing: -0.025em;
      color: var(--md-sys-color-on-surface);
    }
    .menu-btn {
      width: 36px;
      height: 36px;
      border: none;
      border-radius: 50%;
      background: transparent;
      color: var(--md-sys-color-on-surface-variant);
      cursor: pointer;
      display: grid;
      place-items: center;
      padding: 0;
    }
    .menu-btn:hover {
      background: var(--md-sys-color-surface-container);
    }
    .menu-btn svg {
      width: 22px;
      height: 22px;
    }
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
    .avatar {
      width: 34px;
      height: 34px;
      border-radius: 50%;
      background:
        radial-gradient(circle at 35% 30%, oklch(0.92 0.03 80), transparent 45%),
        radial-gradient(circle at 50% 70%, oklch(0.55 0.08 186), oklch(0.42 0.06 186));
      border: 2px solid var(--md-sys-color-outline-variant);
      flex-shrink: 0;
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
      padding: 18px 28px 32px;
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
    @keyframes page-in {
      from {
        opacity: 0;
        transform: translateY(6px);
      }
      to {
        opacity: 1;
        transform: none;
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
  }

  private async loadSettings() {
    const s = await getSettings();
    this.onboarded = !!s.onboarding_complete;
    this.mode = s.mode === "agent" ? "agent" : "standard";
    const navParam = new URLSearchParams(location.search).get("nav");
    const navAlias: Record<string, NavId> = { instance: "instances", library: "instances", store: "download" };
    const nav = (navAlias[navParam ?? ""] ?? navParam) as NavId | null;
    if (nav && ["home", "download", "instances", "multiplayer", "settings", "more", "agent"].includes(nav)) {
      this.route = nav;
    }
    this.ready = true;
  }

  private onOnboarded = () => {
    void this.loadSettings().then(() => {
      this.route = "home";
      this.pageKey += 1;
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
  };

  private goSettings() {
    this.route = "settings";
    this.pageKey += 1;
  }

  render() {
    if (!this.ready) return html``;
    if (!this.onboarded) return html`<easymc-onboarding></easymc-onboarding>`;
    return html`
      <easymc-side-nav .active=${this.route} .mode=${this.mode}></easymc-side-nav>
      <div class="main">
        <header class="top">
          <div class="brand">
            <button class="menu-btn" type="button" title="菜单" aria-label="菜单" tabindex="-1">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <path d="M4 7h16M4 12h16M4 17h16" />
              </svg>
            </button>
            <span>EasyMC</span>
          </div>
          <button class="account" type="button" title="账户" @click=${() => this.goSettings()}>
            <div class="avatar" aria-hidden="true"></div>
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
