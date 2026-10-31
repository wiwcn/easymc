import { LitElement, css, html, nothing } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import type { InstallProgressDto } from "../install-progress";
import { overallPercent, stageLabel } from "../install-progress";

/**
 * One install-progress surface for the whole app.
 *
 * Before this existed every page hand-rolled its own line of text (and the home
 * page its own bar), so the same download looked different depending on where
 * you were standing and the readout was buried at the bottom of a card. This
 * component owns the "what is happening / how far along" contract and is meant
 * to be pinned to the top of the page so it never scrolls out of view.
 */
@customElement("easymc-install-progress")
export class InstallProgress extends LitElement {
  @property({ attribute: false }) progress: InstallProgressDto | null = null;
  /** `banner` (sticky, default) or `inline` (in-flow, no pinning). */
  @property() variant: "banner" | "inline" = "banner";

  @state() private shown: number | null = null;
  /** Highest percentage seen for the current install: the bar never walks back. */
  private peak = 0;

  protected willUpdate() {
    const p = this.progress;
    if (!p) {
      this.peak = 0;
      this.shown = null;
      return;
    }
    const raw = overallPercent(p);
    if (raw == null) {
      this.shown = null;
      return;
    }
    this.peak = Math.max(this.peak, raw);
    this.shown = this.peak;
  }

  static styles = css`
    :host {
      display: block;
      box-sizing: border-box;
      /* Default (no variant attribute) is the sticky banner — pages drop the
         element in bare, so hanging the pin on :host (not on an attribute the
         caller may never set) is what actually keeps it pinned. */
      position: sticky;
      top: 0;
      z-index: 30;
      margin: 0 0 var(--easymc-space-4);
    }
    :host([variant="inline"]) {
      position: static;
      z-index: auto;
      margin: 0;
    }
    .wrap {
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
      padding: var(--easymc-space-3) var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container);
      border: 1px solid
        color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      /* CF-1: border only — no elevation on the same face. */
    }
    :host .wrap {
      /* Inset shadow as a separator only; it is not an elevation tier. */
      box-shadow: 0 1px 0 0 color-mix(in oklch, var(--md-sys-color-outline-variant) 35%, transparent);
    }
    :host([variant="inline"]) .wrap {
      box-shadow: none;
    }
    .row {
      display: flex;
      align-items: center;
      gap: var(--easymc-space-2);
      min-width: 0;
    }
    .stage {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      flex-shrink: 0;
      padding: 3px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
    }
    .dot {
      width: 7px;
      height: 7px;
      border-radius: 50%;
      background: var(--md-sys-color-primary);
      animation: pulse 1.4s ease-in-out infinite;
    }
    .detail {
      flex: 1;
      min-width: 0;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .counts {
      flex-shrink: 0;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      font-variant-numeric: tabular-nums;
    }
    .pct {
      flex-shrink: 0;
      min-width: 3.2ch;
      text-align: right;
      font-size: var(--easymc-type-label);
      font-weight: 700;
      color: var(--md-sys-color-on-surface);
      font-variant-numeric: tabular-nums;
    }
    .track {
      position: relative;
      height: 6px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-surface-container-high);
      overflow: hidden;
    }
    .fill {
      display: block;
      height: 100%;
      width: var(--w, 0%);
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      transition: width var(--easymc-dur-med) var(--easymc-ease-out);
    }
    /* Unknown total → indeterminate sweep so the surface still reads as "working". */
    .fill.indeterminate {
      width: 40%;
      animation: sweep 1.3s var(--easymc-ease-out) infinite;
    }
    @keyframes sweep {
      from { transform: translateX(-110%); }
      to { transform: translateX(280%); }
    }
    @keyframes pulse {
      0%, 100% { opacity: 1; }
      50% { opacity: 0.35; }
    }
    @media (prefers-reduced-motion: reduce) {
      .fill { transition: none; }
      .fill.indeterminate { animation: none; width: 100%; opacity: 0.5; }
      .dot { animation: none; }
    }
  `;

  render() {
    const p = this.progress;
    if (!p) return nothing;

    const failed = p.stage === "error";
    const pct = this.shown;
    const hasTotal = typeof p.total === "number" && p.total > 0;
    const counts = hasTotal
      ? `${p.current} / ${p.total}`
      : p.current > 0
        ? `${p.current}`
        : "";
    const bits = [
      counts,
      p.downloaded ? `新下 ${p.downloaded}` : "",
      p.skipped ? `跳过 ${p.skipped}` : "",
    ].filter(Boolean);

    return html`<div class="wrap" role="status" aria-live="polite">
      <div class="row">
        <span class="stage">
          ${failed ? nothing : html`<span class="dot" aria-hidden="true"></span>`}
          ${stageLabel(p)}
        </span>
        <span class="detail" title=${p.detail}>${p.detail}</span>
        ${bits.length ? html`<span class="counts">${bits.join(" · ")}</span>` : nothing}
        ${pct != null ? html`<span class="pct">${Math.round(pct)}%</span>` : nothing}
      </div>
      <div
        class="track"
        role="progressbar"
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow=${pct != null ? Math.round(pct) : nothing}
      >
        <i
          class="fill ${pct == null ? "indeterminate" : ""}"
          style=${pct != null ? `--w:${pct}%` : ""}
        ></i>
      </div>
    </div>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-install-progress": InstallProgress;
  }
}
