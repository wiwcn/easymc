import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { NavId } from "../types";

const ITEMS: { id: NavId; label: string; icon: string }[] = [
  { id: "home", label: "启动", icon: "⌂" },
  { id: "library", label: "库", icon: "☰" },
  { id: "store", label: "商店", icon: "👜" },
  { id: "agent", label: "Agent", icon: "◎" },
  { id: "share", label: "分享", icon: "⑂" },
  { id: "settings", label: "设置", icon: "⚙" },
];

@customElement("easymc-side-nav")
export class SideNav extends LitElement {
  @property({ type: String }) active: NavId = "home";

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      width: var(--easymc-nav-width);
      padding: 20px 12px;
      background: var(--md-sys-color-surface-container-low);
      border-right: 1px solid var(--md-sys-color-outline-variant);
      flex-shrink: 0;
    }
    nav {
      display: flex;
      flex-direction: column;
      gap: 4px;
      flex: 1;
    }
    button.item {
      display: flex;
      align-items: center;
      gap: 12px;
      border: none;
      background: transparent;
      color: var(--md-sys-color-on-surface-variant);
      padding: 12px 16px;
      border-radius: var(--md-sys-shape-corner-full);
      cursor: pointer;
      font-size: 0.95rem;
      font-weight: 500;
      text-align: left;
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.item:hover {
      background: var(--md-sys-color-surface-container-high);
    }
    button.item:active {
      transform: scale(0.98);
    }
    button.item.active {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
    }
    button.item:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }
    .icon {
      width: 1.25rem;
      text-align: center;
      font-size: 1.1rem;
      line-height: 1;
      opacity: 0.9;
    }
    .promo {
      margin-top: auto;
      padding: 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
      font-size: 0.8rem;
      line-height: 1.4;
    }
    .promo strong {
      display: block;
      font-size: 0.9rem;
      margin-bottom: 4px;
      letter-spacing: -0.01em;
    }
    @media (prefers-reduced-motion: reduce) {
      button.item:active {
        transform: none;
      }
    }
  `;

  render() {
    return html`
      <nav aria-label="主导航">
        ${ITEMS.map(
          (it) => html`
            <button
              class="item ${this.active === it.id ? "active" : ""}"
              type="button"
              aria-current=${this.active === it.id ? "page" : "false"}
              @click=${() => this.select(it.id)}
            >
              <span class="icon" aria-hidden="true">${it.icon}</span>
              <span>${it.label}</span>
            </button>
          `,
        )}
      </nav>
      <div class="promo">
        <strong>1.21.1 就绪</strong>
        多实例、模组与分享包
      </div>
    `;
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
