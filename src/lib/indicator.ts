/**
 * Sliding indicator (MD3 MC-1 / MC-2 / MC-8 · lateral transition §4.3).
 *
 * Positions an absolutely-placed pill so it sits under the active child. Used by
 * `.md-tabs` / `.md-segmented` (`button.on`) and the side-nav rail (`button.active`).
 * Call `syncIndicator(this.renderRoot)` from a Lit component's `updated()`; it
 * re-syncs on container resize.
 */
const roByWrap = new WeakMap<HTMLElement, ResizeObserver>();

/** Place `ind` (absolute, inside `wrap`) over `on`, using offset geometry. */
export function syncIndicatorWithin(
  wrap: HTMLElement,
  indSelector: string,
  onSelector: string,
): void {
  const ind = wrap.querySelector<HTMLElement>(`:scope > ${indSelector}`);
  if (!ind) return;
  const on = wrap.querySelector<HTMLElement>(`:scope > ${onSelector}`);
  if (!on) {
    ind.style.opacity = "0";
    return;
  }
  ind.style.opacity = "1";
  ind.style.width = `${on.offsetWidth}px`;
  ind.style.height = `${on.offsetHeight}px`;
  ind.style.transform = `translate(${on.offsetLeft}px, ${on.offsetTop}px)`;
  if (typeof ResizeObserver !== "undefined" && !roByWrap.has(wrap)) {
    const ro = new ResizeObserver(() => syncIndicatorWithin(wrap, indSelector, onSelector));
    ro.observe(wrap);
    roByWrap.set(wrap, ro);
  }
}

export function syncIndicator(root: ParentNode): void {
  root
    .querySelectorAll<HTMLElement>(".md-tabs, .md-segmented")
    .forEach((w) => syncIndicatorWithin(w, ".md-tab-ind", "button.on"));
}
