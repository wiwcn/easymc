import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";

@customElement("easymc-placeholder-page")
export class PlaceholderPage extends LitElement {
  @property() title = "";
  @property() subtitle = "";
  @property() icon = "📦";

  static styles = css`
    :host {
      display: grid;
      place-items: center;
      min-height: 60vh;
    }
    .box {
      text-align: center;
      max-width: 420px;
      padding: 40px 28px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px dashed var(--md-sys-color-outline-variant);
    }
    .icon {
      font-size: 3rem;
      margin-bottom: 12px;
    }
    h1 {
      margin: 0 0 8px;
      font-size: 1.5rem;
    }
    p {
      margin: 0;
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.5;
    }
  `;

  render() {
    return html`
      <div class="box">
        <div class="icon">${this.icon}</div>
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
