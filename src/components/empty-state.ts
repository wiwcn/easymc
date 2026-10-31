import { LitElement, css, html, nothing } from "lit";
import { customElement, property } from "lit/decorators.js";
import { enterUp } from "../styles/motion";

/**
 * Empty-state block: 2D-centered inside its container.
 * - variant="page"  → fills the main content pane (E1)
 * - variant="panel" → fills a card / list slot (E2–E4)
 */
@customElement("easymc-empty-state")
export class EmptyState extends LitElement {
  @property({ type: String, reflect: true }) variant: "page" | "panel" = "panel";
  @property({ type: String }) title = "";
  @property({ type: String }) description = "";

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      text-align: center;
      box-sizing: border-box;
      width: 100%;
      color: var(--md-sys-color-on-surface-variant);
    }
    :host([variant="page"]) {
      flex: 1 1 auto;
      align-self: stretch;
      min-height: 100%;
      padding: var(--easymc-space-5) var(--easymc-space-5) var(--easymc-space-6);
      /* Optical nudge: true center first, then slightly up (DESIGN / Impeccable). */
      transform: translateY(-4%);
    }
    :host([variant="panel"]) {
      flex: 1 1 auto;
      align-self: stretch;
      min-height: 200px;
      height: 100%;
      padding: var(--easymc-space-6) var(--easymc-space-4);
    }
    .stack {
      display: flex;
      flex-direction: column;
      align-items: center;
      max-width: 28em;
    }
    .title {
      margin: 0 0 8px;
      font-size: var(--easymc-type-title);
      font-weight: var(--easymc-weight-medium);
      letter-spacing: -0.02em;
      color: var(--md-sys-color-on-surface);
      line-height: 1.35;
    }
    :host([variant="page"]) .title {
      font-size: var(--easymc-type-title);
    }
    .desc {
      margin: 0 0 18px;
      font-size: var(--easymc-type-body);
      line-height: 1.55;
      opacity: 0.92;
    }
    .desc:last-child {
      margin-bottom: 0;
    }
    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      justify-content: center;
      align-items: center;
    }
    .actions:empty {
      display: none;
    }
    ::slotted(button.primary),
    ::slotted(button.ghost) {
      /* Pages keep their own button styles; slot only for layout. */
    }
  `;

  /** MI-7: a single fade + 8px rise on first mount (skipped under reduced-motion). */
  protected firstUpdated() {
    enterUp(this.renderRoot.querySelector(".stack"));
  }

  render() {
    return html`
      <div class="stack">
        ${this.title ? html`<div class="title">${this.title}</div>` : nothing}
        ${this.description ? html`<p class="desc">${this.description}</p>` : nothing}
        <div class="actions"><slot></slot></div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-empty-state": EmptyState;
  }
}
