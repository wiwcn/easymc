/**
 * MD3 ripple (§4.7, restrained) — filled launch / primary buttons + side-nav items.
 *
 * A pointer-origin circle that expands on `short4` then fades on `short3`, driven by
 * `motion`. Reduced-motion never fires it. Installed once via event delegation in
 * main.ts so components stay free of per-button wiring.
 */
import { animate } from "motion";
import { prefersReducedMotion } from "./motion";

const RIPPLE_SELECTOR = "button.launch, button.primary, button.item";

function spawn(el: HTMLElement, x: number, y: number): void {
  const rect = el.getBoundingClientRect();
  const radius = Math.hypot(Math.max(x, rect.width - x), Math.max(y, rect.height - y));
  const ink = document.createElement("span");
  ink.className = "md-ripple";
  ink.style.width = `${radius * 2}px`;
  ink.style.height = `${radius * 2}px`;
  ink.style.left = `${x - radius}px`;
  ink.style.top = `${y - radius}px`;
  el.appendChild(ink);

  const finish = () => ink.remove();
  const controls = animate(
    ink,
    { scale: [0, 1], opacity: [0.12, 0.12, 0] },
    { duration: 0.35, ease: "easeOut" },
  );
  controls.finished.then(finish).catch(finish);
}

export function installRipple(target: Document = document): () => void {
  const onPointerDown = (e: PointerEvent) => {
    if (prefersReducedMotion()) return;
    const el = (e.target as Element | null)?.closest?.(RIPPLE_SELECTOR) as HTMLElement | null;
    if (!el || el.hasAttribute("disabled")) return;
    const rect = el.getBoundingClientRect();
    spawn(el, e.clientX - rect.left, e.clientY - rect.top);
  };
  target.addEventListener("pointerdown", onPointerDown);
  return () => target.removeEventListener("pointerdown", onPointerDown);
}
