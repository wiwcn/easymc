/**
 * Motion helpers — thin wrapper over `motion` (motion.dev **vanilla** entry, no React).
 *
 * MD3 Expressive physics: spatial spring for position/size/shape (slight overshoot
 * allowed), effects spring for color/opacity (no overshoot). Start values are from
 * docs/UI-MD3-MOTION-PLAN.md §4.8; calibrate visually in one place here.
 *
 * Degrades safely: when `prefers-reduced-motion: reduce` is set, or the library
 * throws/never loads, final values are applied instantly. Feature code must never
 * depend on the library. Static easing/duration live in `tokens.css`, not here.
 */
import { animate } from "motion";

/** MD3 spatial spring ≈ dampingRatio 0.8 → slight overshoot. */
export const SPRING_SPATIAL = { type: "spring", stiffness: 380, damping: 30 } as const;
/** MD3 effects spring → high stiffness, no overshoot. */
export const SPRING_EFFECTS = { type: "spring", stiffness: 1600, damping: 80 } as const;

/** Seconds (motion uses seconds, tokens use ms). */
export const DUR = {
  short2: 0.1,
  short3: 0.15,
  short4: 0.2,
  medium2: 0.3,
  medium4: 0.4,
} as const;

export function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export type MotionTargets = Record<string, string | number>;
export type SpringKind = "spatial" | "effects";

/** MD3 `emphasized.decelerate` as a motion bezier tuple. */
const MD3_EMPHASIZED_DECELERATE: [number, number, number, number] = [0.05, 0.7, 0.1, 1];
/** MD3 `standard` as a motion bezier tuple. */
const MD3_STANDARD: [number, number, number, number] = [0.2, 0, 0, 1];

/** Apply keyframes with no tween — used for the reduced-motion / fallback path. */
function applyFinal(el: HTMLElement, targets: MotionTargets): void {
  for (const [k, v] of Object.entries(targets)) {
    el.style.setProperty(k.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`), String(v));
  }
}

/**
 * Spring-animate a single element. Returns a cancel handle (no-op when degraded).
 * Spatial kind overshoots slightly; effects kind does not.
 */
export function springTo(
  el: Element | null,
  targets: MotionTargets,
  kind: SpringKind = "spatial",
): () => void {
  if (!el) return () => {};
  const node = el as HTMLElement;
  if (prefersReducedMotion()) {
    applyFinal(node, targets);
    return () => {};
  }
  try {
    const controls = animate(node, targets, kind === "effects" ? SPRING_EFFECTS : SPRING_SPATIAL);
    return () => controls.stop();
  } catch {
    applyFinal(node, targets);
    return () => {};
  }
}

/**
 * One-shot entrance: fade in + a short upward rise (MI-7). Call once per mount.
 * No-ops under reduced-motion, so the element simply appears at rest.
 */
export function enterUp(el: Element | null, distance = 8): void {
  const node = el as HTMLElement | null;
  if (!node || prefersReducedMotion()) return;
  try {
    animate(
      node,
      { opacity: [0, 1], transform: [`translateY(${distance}px)`, "translateY(0px)"] },
      { duration: DUR.medium4, ease: MD3_EMPHASIZED_DECELERATE },
    );
  } catch {
    /* feature code must not depend on the library */
  }
}

/**
 * Odometer-style integer roll (MI-4): animates the element's text from its
 * previously shown value to `to`, in place. Total ≤ medium2; reduced-motion and
 * first paint jump straight to the final value. `format` defaults to rounding.
 */
export function rollNumber(
  el: Element | null,
  to: number,
  format: (n: number) => string = (n) => String(Math.round(n)),
): void {
  const node = el as HTMLElement | null;
  if (!node) return;
  const prev = Number(node.dataset.roll ?? NaN);
  node.dataset.roll = String(to);
  if (prefersReducedMotion() || !Number.isFinite(prev) || prev === to) {
    node.textContent = format(to);
    return;
  }
  try {
    animate(prev, to, {
      duration: DUR.medium2,
      ease: MD3_STANDARD,
      onUpdate: (v: number) => {
        node.textContent = format(v);
      },
    });
  } catch {
    node.textContent = format(to);
  }
}
