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
    font-weight: 600;
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
    font-weight: 700;
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

  /* ——— Skeleton (LD-1) ——— */
  .skel {
    display: block;
    border-radius: var(--md-sys-shape-corner-medium);
    background: linear-gradient(
      90deg,
      var(--md-sys-color-surface-container) 0%,
      var(--md-sys-color-surface-container-high) 45%,
      var(--md-sys-color-surface-container) 90%
    );
    background-size: 200% 100%;
    animation: easymc-skel-shimmer 1.4s var(--easymc-ease-out) infinite;
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
  @keyframes easymc-skel-shimmer {
    0% { background-position: 100% 0; }
    100% { background-position: -100% 0; }
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
    border-radius: 8px;
  }
  input.md-slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 999px;
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
    border-radius: 999px;
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
    font-weight: 650;
    font-size: var(--easymc-type-body);
    color: var(--md-sys-color-on-surface);
  }
  .slider-value .unit {
    margin-left: 2px;
    font-weight: 500;
    font-size: var(--easymc-type-meta);
    color: var(--md-sys-color-on-surface-variant);
  }
`;
