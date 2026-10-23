import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
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
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
      font-weight: 500;
      font-size: 1.25rem;
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
    }
    .menu-btn:hover {
      background: var(--md-sys-color-surface-container);
    }
    .avatar {
      width: 36px;
      height: 36px;
      border-radius: 50%;
      background: linear-gradient(135deg, #80cbc4, #0f766e);
      border: 2px solid var(--md-sys-color-outline-variant);
    }
    .content {
      flex: 1;
      overflow: auto;
      padding: 8px 28px 28px;
    }
  `;

  private onNav = (e: Event) => {
    const detail = (e as CustomEvent<NavId>).detail;
    this.route = detail;
  };

  render() {
    return html`
      <easymc-side-nav .active=${this.route} @nav-change=${this.onNav}></easymc-side-nav>
      <div class="main">
        <header class="top">
          <div class="brand">
            <button class="menu-btn" aria-label="菜单" type="button">☰</button>
            <span>EasyMC</span>
          </div>
          <div class="avatar" title="账户（M0 占位）"></div>
        </header>
        <div class="content">${this.renderPage()}</div>
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
          subtitle="一键导出 / 导入实例配置（M0 占位）"
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
