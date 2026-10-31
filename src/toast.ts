import { friendlyError } from "./lib/friendly-error";

export type ToastOpts = {
  /** Auto-dismiss ms. Short probes use ~1600; default 2800. */
  ms?: number;
  /** Prefer top placement so onboarding foot CTAs stay clickable. */
  placement?: "bottom" | "top";
};

function ensureHost(placement: "bottom" | "top"): HTMLElement {
  const id = placement === "top" ? "toast-host-top" : "toast-host";
  let host = document.getElementById(id);
  if (!host) {
    host = document.createElement("div");
    host.id = id;
    document.body.appendChild(host);
  }
  return host;
}

/** Remove every toast immediately (e.g. when changing onboarding steps). */
export function clearToasts(): void {
  for (const id of ["toast-host", "toast-host-top"]) {
    const host = document.getElementById(id);
    if (!host) continue;
    host.replaceChildren();
  }
}

/**
 * Show a short player toast. Raw invoke/JS errors are mapped via friendlyError
 * when `friendly` is true (default).
 */
export function showToast(
  message: string,
  msOrOpts: number | ToastOpts = 2800,
  friendly = true,
): void {
  const opts: ToastOpts =
    typeof msOrOpts === "number" ? { ms: msOrOpts } : msOrOpts ?? {};
  const ms = opts.ms ?? 2800;
  const placement = opts.placement ?? "bottom";
  const text = friendly ? friendlyError(message, message) : String(message ?? "");
  if (!text) return;

  const host = ensureHost(placement);
  const el = document.createElement("div");
  el.className = "toast";
  el.textContent = text;
  host.appendChild(el);
  window.setTimeout(() => {
    el.classList.add("out");
    window.setTimeout(() => el.remove(), 220);
  }, ms);
}

/** Toast helper that always runs the message through friendlyError first. */
export function showFriendlyToast(e: unknown, prefix = "", ms = 2800): void {
  const body = friendlyError(e);
  showToast(prefix ? `${prefix}${body}` : body, ms, false);
}
