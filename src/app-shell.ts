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
      padding: 12px 28px;
      min-height: 56px;
      border-bottom: 1px solid
        color-mix(in srgb, var(--md-sys-color-outline-variant) 55%, transparent);
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
      font-weight: 600;
      font-size: 1.2rem;
      letter-spacing: -0.02em;
      color: var(--md-sys-color-on-surface);
    }
    .avatar {
      width: 36px;
      height: 36px;
      border-radius: 50%;
      background: var(--md-sys-color-primary-container);
      border: 2px solid var(--md-sys-color-outline-variant);
      box-shadow: inset 0 0 0 6px
        color-mix(in srgb, var(--md-sys-color-primary) 35%, transparent);
    }
    .content {
      flex: 1;
      overflow: auto;
      padding: 8px 28px 28px;
    }
    .page {
      animation: page-in var(--easymc-dur-page) var(--easymc-ease-out) both;
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
    // legacy redirects from Phase 1 routes
    const map: Record<string, NavId> = {
      library: "instances",
      store: "download",
      share: "more",
    };
    this.route = (map[detail] as NavId) || detail;
    this.pageKey += 1;
  };

  render() {
    if (!this.ready) return html``;
    if (!this.onboarded) return html`<easymc-onboarding></easymc-onboarding>`;
    return html`
      <easymc-side-nav .active=${this.route} .mode=${this.mode}></easymc-side-nav>
      <div class="main">
        <header class="top">
          <div class="brand">
            <span>EasyMC</span>
          </div>
          <div class="avatar" title="账户"></div>
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
