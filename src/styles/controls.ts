/**
 * Shared control vocabulary (CMP-1) + surface craft (CF-1 helpers).
 * Import into Lit `static styles = [controlStyles, css\`...\`]`.
 * Variants: filled (primary) · tonal · outline (ghost) · text · danger · launch (signature only).
 */
import { css } from "lit";

export const controlStyles = css`
  /* ——— Buttons ——— */
  button.primary,
  button.tonal,
  button.ghost,
  button.text,
  button.danger,
  button.launch {
    font-family: inherit;
    font-weight: var(--easymc-weight-medium);
    font-size: var(--easymc-type-label);
    line-height: 1.25;
    letter-spacing: -0.01em;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--easymc-space-2);
    border-radius: var(--md-sys-shape-corner-full);
    padding: 8px 16px;
    transition:
      filter var(--easymc-dur-press) var(--easymc-ease-out),
      background var(--easymc-dur-press) var(--easymc-ease-out),
      color var(--easymc-dur-press) var(--easymc-ease-out),
      border-color var(--easymc-dur-press) var(--easymc-ease-out),
      transform var(--easymc-dur-press) var(--easymc-ease-out),
      box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
  }

  button.primary {
    border: none;
    background: var(--md-sys-color-primary);
    color: var(--md-sys-color-on-primary);
  }
  button.primary:hover:not(:disabled) {
    filter: brightness(1.05);
  }

  button.tonal {
    border: none;
    background: var(--md-sys-color-secondary-container);
    color: var(--md-sys-color-on-secondary-container);
  }
  button.tonal:hover:not(:disabled) {
    filter: brightness(0.97);
  }

  button.ghost {
    border: 1px solid var(--md-sys-color-outline-variant);
    background: transparent;
    color: var(--md-sys-color-on-surface);
    padding: 8px 14px;
  }
  button.ghost:hover:not(:disabled) {
    background: var(--md-sys-color-surface-container);
  }

  button.text {
    border: none;
    background: transparent;
    color: var(--md-sys-color-primary);
    padding: 6px 8px;
  }
  button.text:hover:not(:disabled) {
    background: color-mix(in oklch, var(--md-sys-color-primary) 10%, transparent);
  }

  button.danger {
    border: none;
    background: var(--md-sys-color-error);
    color: var(--md-sys-color-on-error);
  }

  /* Signature fill — ONLY launch CTA (§8.2). Do not reuse elsewhere. */
  button.launch {
    border: none;
    background: var(--easymc-launch-gradient);
    color: var(--md-sys-color-on-primary);
    font-size: var(--easymc-type-title);
    font-weight: var(--easymc-weight-bold);
    padding: 15px 28px;
    box-shadow:
      var(--md-sys-elevation-2),
      0 1px 0 oklch(1 0 0 / 0.18) inset;
  }
  button.launch:hover:not(:disabled) {
    filter: brightness(1.06);
  }
  button.launch:active:not(:disabled) {
    transform: scale(0.985);
  }
  button.launch:disabled {
    opacity: 0.55;
    cursor: not-allowed;
    transform: none;
  }
  button.launch svg {
    width: 20px;
    height: 20px;
  }

  button.primary:disabled,
  button.tonal:disabled,
  button.ghost:disabled,
  button.text:disabled,
  button.danger:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  /* ——— State layer (MD3 §4.2): hover 8 / focus 12 / pressed 12 / dragged 16 ———
     A currentColor overlay painted below the label (negative z-index) instead of
     each page hand-rolling its own background swap. Opt in on any interactive
     surface (nav item, row, card) by adding the md-state class. */
  button.primary,
  button.tonal,
  button.ghost,
  button.text,
  button.danger,
  button.launch,
  .md-state {
    position: relative;
    isolation: isolate;
  }
  button.primary::after,
  button.tonal::after,
  button.ghost::after,
  button.text::after,
  button.danger::after,
  button.launch::after,
  .md-state::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    background: currentColor;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--easymc-dur-press) var(--easymc-ease-out);
  }
  button.primary:hover:not(:disabled)::after,
  button.tonal:hover:not(:disabled)::after,
  button.ghost:hover:not(:disabled)::after,
  button.text:hover:not(:disabled)::after,
  button.danger:hover:not(:disabled)::after,
  button.launch:hover:not(:disabled)::after,
  .md-state:hover:not(:disabled)::after {
    opacity: var(--md-sys-state-hover-opacity);
  }
  button.primary:focus-visible::after,
  button.tonal:focus-visible::after,
  button.ghost:focus-visible::after,
  button.text:focus-visible::after,
  button.danger:focus-visible::after,
  button.launch:focus-visible::after,
  .md-state:focus-visible::after {
    opacity: var(--md-sys-state-focus-opacity);
  }
  button.primary:active:not(:disabled)::after,
  button.tonal:active:not(:disabled)::after,
  button.ghost:active:not(:disabled)::after,
  button.text:active:not(:disabled)::after,
  button.danger:active:not(:disabled)::after,
  button.launch:active:not(:disabled)::after,
  .md-state:active:not(:disabled)::after {
    opacity: var(--md-sys-state-pressed-opacity);
  }
  /* Ripple layer (MD3 §4.7, restrained) — filled launch/primary + nav only.
     A pointer-origin circle that expands then fades; reduced-motion never fires
     (see styles/ripple.ts — it bails before creating the element). */
  .md-ripple {
    position: absolute;
    border-radius: 50%;
    background: currentColor;
    opacity: 0;
    pointer-events: none;
    transform: scale(0);
    z-index: -1;
  }
  /* Clip the ripple to the (pill) shape on ripple-enabled surfaces. */
  button.launch,
  button.primary {
    overflow: hidden;
  }

  /* ——— Tabs + Segmented (MD3 MC-1 / MC-2) — one vocabulary, sliding indicator.
     Markup: <div class="md-tabs"><span class="md-tab-ind"></span><button class="on">…</button></div>
     The indicator position is driven by lib/indicator.ts (lateral, §4.3). */
  .md-tabs,
  .md-segmented {
    position: relative;
    display: flex;
    gap: 2px;
    /* LD-2: one scrolling row instead of wrapping. A wrapped set would split
       the sliding indicator's track in two and read as broken at narrow
       widths, so the row scrolls (scrollbar hidden — it is a pill). */
    flex-wrap: nowrap;
    overflow-x: auto;
    overscroll-behavior-x: contain;
    align-items: center;
    padding: 3px;
    margin-bottom: var(--easymc-space-4);
    border-radius: var(--md-sys-shape-corner-full);
    background: var(--md-sys-color-surface-container-low);
    width: fit-content;
    max-width: 100%;
    isolation: isolate;
    scrollbar-width: none;
  }
  .md-tabs::-webkit-scrollbar,
  .md-segmented::-webkit-scrollbar {
    display: none;
  }
  .md-tabs .md-tab-ind,
  .md-segmented .md-tab-ind {
    position: absolute;
    left: 0;
    top: 0;
    z-index: -1;
    border-radius: var(--md-sys-shape-corner-full);
    background: var(--md-sys-color-secondary-container);
    opacity: 0;
    pointer-events: none;
    transition:
      transform var(--easymc-dur-med) var(--easymc-ease-out),
      width var(--easymc-dur-med) var(--easymc-ease-out),
      height var(--easymc-dur-med) var(--easymc-ease-out),
      opacity var(--easymc-dur-fast) var(--easymc-ease-out);
  }
  .md-tabs button,
  .md-segmented button {
    position: relative;
    border: none;
    background: transparent;
    cursor: pointer;
    flex: 0 0 auto; /* keep label width so the row scrolls instead of squashing */
    white-space: nowrap;
    padding: 8px 16px;
    /* Explicit line box: Latin and CJK labels otherwise get different "normal"
       line heights, so a short Latin tab would be a few px shorter than its
       CJK neighbours and the sliding indicator would jump height when it lands. */
    line-height: 20px;
    border-radius: var(--md-sys-shape-corner-full);
    color: var(--md-sys-color-on-surface-variant);
    font-family: inherit;
    font-size: var(--easymc-type-label);
    font-weight: var(--easymc-weight-medium);
    transition: color var(--easymc-dur-press) var(--easymc-ease-out);
  }
  .md-tabs button.on,
  .md-segmented button.on {
    color: var(--md-sys-color-on-secondary-container);
    font-weight: var(--easymc-weight-medium);
  }
  .md-tabs button:focus-visible,
  .md-segmented button:focus-visible {
    outline: none;
    box-shadow: var(--easymc-focus-ring);
  }
  .md-segmented button {
    min-width: 96px;
    justify-content: center;
  }

  /* ——— Chips (MD3 MC-4) — filter / assist — min 32px, unified type ——— */
  .md-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--easymc-space-2);
    min-height: 32px;
    padding: 0 12px;
    border: 1px solid var(--md-sys-color-outline-variant);
    border-radius: var(--md-sys-shape-corner-full);
    background: transparent;
    color: var(--md-sys-color-on-surface-variant);
    font-family: inherit;
    font-size: var(--easymc-type-label);
    font-weight: var(--easymc-weight-medium);
    cursor: pointer;
    transition:
      background var(--easymc-dur-press) var(--easymc-ease-out),
      border-color var(--easymc-dur-press) var(--easymc-ease-out),
      color var(--easymc-dur-press) var(--easymc-ease-out);
  }
  .md-chip.on {
    border-color: transparent;
    background: var(--md-sys-color-secondary-container);
    color: var(--md-sys-color-on-secondary-container);
  }

  /* ——— Checkbox / Radio (MD3 MC-3) — native control, MD3 tint + hit area ——— */
  input[type="checkbox"],
  input[type="radio"] {
    accent-color: var(--md-sys-color-primary);
    width: 18px;
    height: 18px;
    cursor: pointer;
  }
  .md-check {
    display: inline-flex;
    align-items: center;
    gap: var(--easymc-space-2);
    min-height: 24px;
    cursor: pointer;
  }

  @media (prefers-reduced-motion: reduce) {
    .md-tabs .md-tab-ind,
    .md-segmented .md-tab-ind {
      transition: none;
    }
  }

  /* ——— Surfaces (CF-1: elevation XOR border) ——— */
  .surf-elev {
    background: var(--md-sys-color-surface-container-lowest);
    border-radius: var(--md-sys-shape-corner-extra-large);
    box-shadow: var(--md-sys-elevation-1);
    border: none;
  }
  .surf-outline {
    background: var(--md-sys-color-surface-container-lowest);
    border-radius: var(--md-sys-shape-corner-extra-large);
    border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
    box-shadow: none;
  }
  .surf-elev-2 {
    background: var(--md-sys-color-surface-container-lowest);
    border-radius: var(--md-sys-shape-corner-medium);
    box-shadow: var(--md-sys-elevation-2);
    border: none;
  }

  /* ——— Skeleton (LD-1 · C-5: MD3 pulse, not a shimmer sweep) ——— */
  .skel {
    display: block;
    border-radius: var(--md-sys-shape-corner-medium);
    background: var(--md-sys-color-surface-container-high);
    animation: easymc-skel-pulse 1.5s var(--easymc-ease-out) infinite;
  }
  .skel-line {
    height: 0.9em;
    margin: 0.35em 0;
  }
  .skel-line.w40 { width: 40%; }
  .skel-line.w60 { width: 60%; }
  .skel-line.w80 { width: 80%; }
  .skel-line.w100 { width: 100%; }
  .skel-block {
    width: 100%;
  }
  @keyframes easymc-skel-pulse {
    0%, 100% { background-color: var(--md-sys-color-surface-container-high); }
    50% { background-color: var(--md-sys-color-surface-container-highest); }
  }
  @media (prefers-reduced-motion: reduce) {
    .skel {
      animation: none;
      background: var(--md-sys-color-surface-container);
    }
  }

  /* ——— Sliders (MD3: track + side value) ——— */
  .slider-row {
    display: flex;
    align-items: center;
    gap: var(--easymc-space-3);
    width: 100%;
  }
  input.md-slider,
  input[type="range"].md-slider {
    -webkit-appearance: none;
    appearance: none;
    flex: 1 1 auto;
    min-width: 0;
    height: 40px;
    margin: 0;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 0;
    cursor: pointer;
  }
  input.md-slider:focus-visible {
    outline: 2px solid var(--md-sys-color-primary);
    outline-offset: 4px;
    border-radius: var(--md-sys-shape-corner-small);
  }
  input.md-slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: var(--md-sys-shape-corner-full);
    background: var(--md-sys-color-surface-container-highest);
  }
  input.md-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 20px;
    height: 20px;
    margin-top: -8px;
    border-radius: 50%;
    background: var(--md-sys-color-primary);
    border: none;
    box-shadow: var(--md-sys-elevation-1);
  }
  input.md-slider::-moz-range-track {
    height: 4px;
    border-radius: var(--md-sys-shape-corner-full);
    background: var(--md-sys-color-surface-container-highest);
    border: none;
  }
  input.md-slider::-moz-range-thumb {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--md-sys-color-primary);
    border: none;
    box-shadow: var(--md-sys-elevation-1);
  }
  .slider-value {
    flex: 0 0 auto;
    min-width: 4.5rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-weight: var(--easymc-weight-medium);
    font-size: var(--easymc-type-body);
    color: var(--md-sys-color-on-surface);
  }
  .slider-value .unit {
    margin-left: 2px;
    font-weight: var(--easymc-weight-medium);
    font-size: var(--easymc-type-meta);
    color: var(--md-sys-color-on-surface-variant);
  }
`;

/** Shared page chrome — titles, subs, section heads (TYP-1 / SP-1). */
export const pageChromeStyles = css`
  h1 {
    margin: 0 0 var(--easymc-space-1);
    font-size: var(--easymc-type-display);
    font-weight: var(--easymc-weight-bold);
    letter-spacing: -0.03em;
    color: var(--md-sys-color-on-surface);
    line-height: 1.2;
  }
  .sub,
  p.sub {
    margin: 0 0 var(--easymc-space-4);
    font-size: var(--easymc-type-label);
    color: var(--md-sys-color-on-surface-variant);
    line-height: 1.45;
  }
  .section-head h2,
  h2.section-title {
    margin: 0;
    font-size: var(--easymc-type-title);
    font-weight: var(--easymc-weight-medium);
    letter-spacing: -0.02em;
  }
  .muted {
    color: var(--md-sys-color-on-surface-variant);
    font-size: var(--easymc-type-meta);
  }
`;


/** Shared paper / glass cards + responsive grids (LD-2 · CF-1).
 *  Prefer these tokens over per-page one-offs when unifying rhythm. */
export const cardSurfaceStyles = css`
  .em-paper {
    padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
    border-radius: var(--easymc-card-radius);
    background: var(--md-sys-color-surface-container-lowest);
    box-shadow: var(--md-sys-elevation-1);
    border: none;
    box-sizing: border-box;
    min-width: 0;
  }
  .em-glass {
    padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
    border-radius: var(--easymc-card-radius);
    background: var(--easymc-glass-surface);
    backdrop-filter: blur(var(--easymc-glass-blur));
    -webkit-backdrop-filter: blur(var(--easymc-glass-blur));
    box-shadow: var(--md-sys-elevation-1);
    border: none;
    box-sizing: border-box;
    min-width: 0;
  }
  @media (prefers-reduced-transparency: reduce) {
    .em-glass {
      background: var(--md-sys-color-surface-container-lowest);
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }
  .em-grid-dense {
    display: grid;
    grid-template-columns: repeat(
      auto-fill,
      minmax(min(var(--easymc-grid-min-dense), 100%), 1fr)
    );
    gap: var(--easymc-grid-gap);
    align-items: stretch;
  }
  .em-grid-tile {
    display: grid;
    grid-template-columns: repeat(
      auto-fill,
      minmax(min(var(--easymc-grid-min-tile), 100%), 1fr)
    );
    gap: var(--easymc-grid-gap);
    align-items: stretch;
  }

  /* Narrow page column: drop track mins so 1024-wide shells still get 2–3 cols
     without horizontal overflow or single stretched cards. */
  @container easymc-page (max-width: 720px) {
    .em-grid-dense {
      grid-template-columns: repeat(
        auto-fill,
        minmax(min(var(--easymc-grid-min-dense-narrow), 100%), 1fr)
      );
    }
    .em-grid-tile {
      grid-template-columns: repeat(
        auto-fill,
        minmax(min(var(--easymc-grid-min-tile-narrow), 100%), 1fr)
      );
    }
  }

  /* Lighter paper — vanilla version tiles / library instance cards (Operate). */
  .em-paper-lite {
    padding: var(--easymc-space-3) var(--easymc-space-3);
    border-radius: var(--easymc-card-radius);
    background: var(--md-sys-color-surface-container-low);
    border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
    box-shadow: none;
    box-sizing: border-box;
    min-width: 0;
  }
`;
