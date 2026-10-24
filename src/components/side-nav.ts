import { LitElement, css, html, nothing, svg } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { AppMode, NavId } from "../types";

type Item = { id: NavId; label: string; icon: string };

const ICONS: Record<string, unknown> = {
  home: svg`<path d="M4 10.5 12 4l8 6.5V20a1 1 0 0 1-1 1h-5v-6H10v6H5a1 1 0 0 1-1-1v-9.5Z" fill="currentColor"/>`,
  download: svg`<path d="M12 3v10m0 0 4-4m-4 4-4-4M5 17v2a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-2" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>`,
  multiplayer: svg`<path d="M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm8 0a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM4.5 19c.6-2.2 2.6-3.5 5.5-3.5s4.9 1.3 5.5 3.5M14 15.5c1 .3 2.1.5 3.5.5 2.4 0 4-.8 4.5 2" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round"/>`,
  settings: svg`<path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" stroke="currentColor" stroke-width="1.8" fill="none"/><path d="M19.4 13a7.8 7.8 0 0 0 .1-2l1.7-1.3-1.7-3-2 .5a7.6 7.6 0 0 0-1.7-1L15.4 3h-3.8l-.4 2.2a7.6 7.6 0 0 0-1.7 1l-2-.5-1.7 3L7.5 11a7.8 7.8 0 0 0 0 2l-1.7 1.3 1.7 3 2-.5c.5.4 1.1.8 1.7 1l.4 2.2h3.8l.4-2.2c.6-.2 1.2-.6 1.7-1l2 .5 1.7-3L19.4 13Z" stroke="currentColor" stroke-width="1.4" fill="none"/>`,
  more: svg`<circle cx="6" cy="12" r="1.6" fill="currentColor"/><circle cx="12" cy="12" r="1.6" fill="currentColor"/><circle cx="18" cy="12" r="1.6" fill="currentColor"/>`,
  agent: svg`<path d="M8 10h8M9 14h6M7 7h10a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H13l-3 3v-3H7a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z" stroke="currentColor" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round"/>`,
};

const BASE: Item[] = [
  { id: "home", label: "启动", icon: "home" },
  { id: "download", label: "下载", icon: "download" },
  { id: "multiplayer", label: "联机", icon: "multiplayer" },
  { id: "settings", label: "设置", icon: "settings" },
  { id: "more", label: "更多", icon: "more" },
];

function itemsFor(mode: AppMode): Item[] {
  const agent: Item = { id: "agent", label: "助手", icon: "agent" };
  if (mode === "agent") {
    // Agent mode: 助手 sits after 启动
    return [BASE[0], agent, ...BASE.slice(1)];
  }
  // Standard: 助手 lives under 更多 — no top-level agent item
  return BASE;
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
      font-size: 0.7rem;
      padding: 2px 8px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
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
              class="item ${this.active === it.id ? "active" : ""}"
              type="button"
              aria-current=${this.active === it.id ? "page" : "false"}
              @click=${() => this.select(it.id)}
            >
              <span class="ico" aria-hidden="true"
                ><svg viewBox="0 0 24 24">${ICONS[it.icon]}</svg></span
              >
              <span>${it.label}</span>
              ${it.id === "agent" && this.mode === "agent"
                ? html`<span class="pill">已开启</span>`
                : nothing}
            </button>
          `,
        )}
      </nav>
      <div class="promo">
        <strong>${this.mode === "agent" ? "助手已就绪" : "准备好了就开玩"}</strong>
        ${this.mode === "agent"
          ? "游戏打不开或崩溃时，打开助手帮你看看"
          : "选好版本，点启动游戏，就能玩"}
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
