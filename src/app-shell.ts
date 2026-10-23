import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import { keyed } from "lit/directives/keyed.js";
import type { NavId } from "./types";
import "./components/side-nav";
import "./pages/home-page";
import "./pages/library-page";
import "./pages/placeholder-page";
import "./pages/store-page";
import "./pages/agent-page";

@customElement("easymc-app")
export class EasyMcApp extends LitElement {
  @state() private route: NavId = "home";
  @state() private pageKey = 0;

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
    .menu-btn {
      width: 40px;
      height: 40px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      background: transparent;
      color: var(--md-sys-color-on-surface-variant);
      cursor: pointer;
      font-size: 1.25rem;
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .menu-btn:hover {
      background: var(--md-sys-color-surface-container);
    }
    .menu-btn:active {
      transform: scale(0.96);
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
      .menu-btn:active {
        transform: none;
      }
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    this.addEventListener("nav-change", this.onNav as EventListener);
  }

  disconnectedCallback() {
    this.removeEventListener("nav-change", this.onNav as EventListener);
    super.disconnectedCallback();
  }

  private onNav = (e: Event) => {
    const detail = (e as CustomEvent<NavId>).detail;
    if (!detail || detail === this.route) return;
    this.route = detail;
    this.pageKey += 1;
  };

  render() {
    return html`
      <easymc-side-nav .active=${this.route}></easymc-side-nav>
      <div class="main">
        <header class="top">
          <div class="brand">
            <button class="menu-btn" aria-label="菜单" type="button">☰</button>
            <span>EasyMC</span>
          </div>
          <div class="avatar" title="账户（M0 占位）"></div>
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
      case "library":
        return html`<easymc-library-page></easymc-library-page>`;
      case "store":
        return html`<easymc-store-page></easymc-store-page>`;
      case "agent":
        return html`<easymc-agent-page></easymc-agent-page>`;
      case "share":
        return html`<easymc-placeholder-page
          title="分享"
          subtitle="在「库」或「启动」页导出元数据 / 整合包，或导入 .json/.zip（M1.7）"
          icon="🔗"
        ></easymc-placeholder-page>`;
      case "settings":
        return html`<easymc-placeholder-page
          title="设置"
          subtitle="账户与运行时"
          icon="⚙️"
          settings
        ></easymc-placeholder-page>`;
    }
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-app": EasyMcApp;
  }
}
