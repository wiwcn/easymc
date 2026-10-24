import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import { pageChromeStyles } from "../styles/controls";

/**
 * Dead / legacy route stub (CPY-1). Not mounted on main app-shell routes.
 * Kept so old imports do not break; no developer-env copy.
 */
@customElement("easymc-placeholder-page")
export class PlaceholderPage extends LitElement {
  @property() title = "即将推出";
  @property() subtitle = "这一页还在准备中。";

  static styles = [
    pageChromeStyles,
    css`
      :host {
        display: flex;
        flex: 1 0 auto;
        min-height: 100%;
        width: 100%;
        align-items: center;
        justify-content: center;
        box-sizing: border-box;
      }
      .box {
        text-align: center;
        max-width: 28em;
        padding: var(--easymc-space-7) var(--easymc-space-5);
        border-radius: var(--md-sys-shape-corner-extra-large);
        background: var(--md-sys-color-surface-container-low);
        border: 1px dashed var(--md-sys-color-outline-variant);
        box-shadow: none;
      }
      h1 { margin: 0 0 var(--easymc-space-2); }
      p {
        margin: 0;
        color: var(--md-sys-color-on-surface-variant);
        line-height: 1.5;
        font-size: var(--easymc-type-body);
      }
    `,
  ];

  render() {
    return html`
      <div class="box">
        <h1>${this.title}</h1>
        <p>${this.subtitle}</p>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-placeholder-page": PlaceholderPage;
  }
}
