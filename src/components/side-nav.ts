import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { AppMode, NavId } from "../types";

type Item = { id: NavId; label: string };
const BASE: Item[] = [
  { id: "home", label: "启动" },
  { id: "library", label: "库" },
  { id: "store", label: "商店" },
  { id: "share", label: "分享" },
  { id: "settings", label: "设置" },
];
const AGENT: Item = { id: "agent", label: "助手" };

/** Agent mode: assistant sits right after 启动 with a badge. Standard: last, de-emphasised. */
function itemsFor(mode: AppMode): Item[] {
  if (mode === "agent") return [BASE[0], AGENT, ...BASE.slice(1)];
  return [...BASE, AGENT];
}

@customElement("easymc-side-nav")
export class SideNav extends LitElement {
  @property({ type: String }) active: NavId = "home";
  @property({ type: String }) mode: AppMode = "standard";

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
    .pill {
      margin-left: auto;
      font-size: 0.7rem;
      padding: 2px 8px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    button.item.quiet {
      font-size: 0.88rem;
      opacity: 0.8;
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
        ${itemsFor(this.mode).map(
          (it) => html`
            <button
              class="item ${this.active === it.id ? "active" : ""} ${it.id === "agent" && this.mode === "standard" ? "quiet" : ""}"
              type="button"
              aria-current=${this.active === it.id ? "page" : "false"}
              @click=${() => this.select(it.id)}
            >
              <span>${it.label}</span>
              ${it.id === "agent" && this.mode === "agent" ? html`<span class="pill">已开启</span>` : null}
            </button>
          `,
        )}
      </nav>
      <div class="promo">
        <strong>${this.mode === "agent" ? "助手已就绪" : "准备好了就开始"}</strong>
        ${this.mode === "agent" ? "游戏出问题时，打开助手帮你看看" : "多实例、模组，一键分享给好友"}
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
