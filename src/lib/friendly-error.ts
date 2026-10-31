/**
 * Map raw invoke / JS errors to player-facing Chinese copy.
 * Never surface `__TAURI__`, `invoke`, or `Cannot read properties…` to players.
 */
export function friendlyError(e: unknown, fallback = "操作没成功，请稍后重试"): string {
  const s =
    e instanceof Error
      ? e.message || e.name || String(e)
      : typeof e === "string"
        ? e
        : e == null
          ? ""
          : String(e);

  const raw = s.trim();
  if (!raw) return fallback;

  // Desktop-shell / Tauri bridge missing (browser preview, broken IPC).
  if (
    /__TAURI__|__TAURI_INTERNALS__|tauri/i.test(raw) ||
    /\binvoke\b/i.test(raw) ||
    /Cannot read propert(?:y|ies) of undefined/i.test(raw) ||
    /null is not an object/i.test(raw) ||
    /is not a function/i.test(raw) && /invoke/i.test(raw)
  ) {
    return "请在 EasyMC 桌面窗口里操作";
  }

  // Generic JS noise that should never reach players.
  if (
    /Cannot read propert/i.test(raw) ||
    /undefined is not/i.test(raw) ||
    /\[object \w+\]/.test(raw) ||
    /^Error:/i.test(raw) && raw.length < 24
  ) {
    return fallback;
  }

  // Already Chinese / short product copy — keep.
  if (/[\u4e00-\u9fff]/.test(raw) && !/invoke|__TAURI/i.test(raw)) {
    return raw.length > 160 ? `${raw.slice(0, 157)}…` : raw;
  }

  // Unknown English / stack-ish → soft fallback.
  if (/at\s+\S+\s+\(|TypeError|ReferenceError|NetworkError|Failed to fetch/i.test(raw)) {
    return fallback;
  }

  return raw.length > 120 ? fallback : raw;
}

/** True when the message looks like a desktop-bridge / preview miss. */
export function isDesktopBridgeError(e: unknown): boolean {
  const s = e instanceof Error ? e.message : String(e ?? "");
  return /__TAURI__|__TAURI_INTERNALS__|\binvoke\b|Cannot read propert(?:y|ies) of undefined/i.test(
    s,
  );
}
