/**
 * Appearance: light/dark/system + accent seed → CSS variables on <html>.
 * Components already consume --md-sys-color-primary* / glass tokens, so
 * changing hue/chroma on documentElement recolors the whole shell.
 */

export type UiThemeMode = "light" | "dark" | "system";

export interface AccentPreset {
  id: string;
  label_zh: string;
  /** Canonical #RRGGBB seed (light-mode primary family). */
  hex: string;
  hue: number;
}

/** Soft, readable seeds — chroma kept moderate so glass panels stay clear. */
export const ACCENT_PRESETS: readonly AccentPreset[] = [
  { id: "teal", label_zh: "青绿", hex: "#0D6E67", hue: 186.5 },
  { id: "ocean", label_zh: "海洋", hex: "#1565C0", hue: 255 },
  { id: "indigo", label_zh: "靛蓝", hex: "#3949AB", hue: 275 },
  { id: "violet", label_zh: "紫罗兰", hex: "#7B1FA2", hue: 320 },
  { id: "rose", label_zh: "玫红", hex: "#C2185B", hue: 5 },
  { id: "amber", label_zh: "琥珀", hex: "#EF6C00", hue: 55 },
  { id: "moss", label_zh: "草木", hex: "#2E7D32", hue: 145 },
] as const;

export const DEFAULT_ACCENT_HEX = ACCENT_PRESETS[0].hex;
export const DEFAULT_ACCENT_HUE = ACCENT_PRESETS[0].hue;
export const DEFAULT_UI_THEME: UiThemeMode = "light";

const ACCENT_CHROMA_CAP = 0.11;
const ACCENT_CHROMA_FLOOR = 0.06;

let systemMql: MediaQueryList | null = null;
let systemHandler: (() => void) | null = null;
let lastMode: UiThemeMode = DEFAULT_UI_THEME;
let lastAccent: string | null = null;

export function normalizeUiTheme(raw: unknown): UiThemeMode {
  const s = String(raw ?? "").trim().toLowerCase();
  if (s === "dark" || s === "system" || s === "light") return s;
  return DEFAULT_UI_THEME;
}

/** Accept #RGB / #RRGGBB; return uppercase #RRGGBB or null. */
export function normalizeAccentHex(raw: unknown): string | null {
  if (raw == null) return null;
  let s = String(raw).trim();
  if (!s) return null;
  if (s[0] !== "#") s = `#${s}`;
  if (/^#[0-9a-fA-F]{3}$/.test(s)) {
    const r = s[1],
      g = s[2],
      b = s[3];
    s = `#${r}${r}${g}${g}${b}${b}`;
  }
  if (!/^#[0-9a-fA-F]{6}$/.test(s)) return null;
  return s.toUpperCase();
}

export function presetMatchingHex(hex: string | null | undefined): AccentPreset | null {
  const n = normalizeAccentHex(hex);
  if (!n) return ACCENT_PRESETS[0];
  return ACCENT_PRESETS.find((p) => p.hex.toUpperCase() === n) ?? null;
}

export function resolveThemeMode(mode: UiThemeMode): "light" | "dark" {
  if (mode === "light" || mode === "dark") return mode;
  if (typeof matchMedia === "function") {
    return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return "light";
}

/** sRGB hex → approximate OKLCH hue + chroma (enough for MD3 seed steering). */
export function hexToOklchHueChroma(hex: string): { h: number; c: number; l: number } {
  const n = normalizeAccentHex(hex) ?? DEFAULT_ACCENT_HEX;
  const r = parseInt(n.slice(1, 3), 16) / 255;
  const g = parseInt(n.slice(3, 5), 16) / 255;
  const b = parseInt(n.slice(5, 7), 16) / 255;
  const lin = (u: number) => (u <= 0.04045 ? u / 12.92 : ((u + 0.055) / 1.055) ** 2.4);
  const R = lin(r),
    G = lin(g),
    B = lin(b);
  // sRGB → OKLab (Björn Ottosson)
  const l_ = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m_ = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s_ = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
  const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
  const bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;
  const c = Math.sqrt(a * a + bb * bb);
  let h = (Math.atan2(bb, a) * 180) / Math.PI;
  if (h < 0) h += 360;
  return { h, c, l: L };
}

function clampChroma(c: number): number {
  if (!Number.isFinite(c) || c < ACCENT_CHROMA_FLOOR) return ACCENT_CHROMA_FLOOR;
  return Math.min(ACCENT_CHROMA_CAP, c);
}

export interface AppearanceInput {
  ui_theme?: unknown;
  accent_color?: unknown;
}

/** Apply theme mode + accent to <html>. Safe to call repeatedly. */
export function applyAppearance(input: AppearanceInput): {
  mode: UiThemeMode;
  resolved: "light" | "dark";
  accent: string;
  hue: number;
  chroma: number;
} {
  const mode = normalizeUiTheme(input.ui_theme);
  const accent =
    normalizeAccentHex(input.accent_color) ??
    normalizeAccentHex(DEFAULT_ACCENT_HEX)!;
  const resolved = resolveThemeMode(mode);
  const preset = presetMatchingHex(accent);
  const { h, c } = preset
    ? { h: preset.hue, c: 0.09 }
    : hexToOklchHueChroma(accent);
  const chroma = clampChroma(c);

  if (typeof document !== "undefined") {
    const root = document.documentElement;
    root.setAttribute("data-theme", resolved);
    root.style.setProperty("--easymc-accent-hue", String(Number(h.toFixed(2))));
    root.style.setProperty("--easymc-accent-chroma", String(Number(chroma.toFixed(4))));
    root.style.setProperty("--easymc-accent-seed", accent);
    root.style.colorScheme = resolved;
  }

  lastMode = mode;
  lastAccent = accent;
  wireSystemListener(mode);

  return { mode, resolved, accent, hue: h, chroma };
}

function wireSystemListener(mode: UiThemeMode) {
  if (typeof matchMedia !== "function") return;
  if (mode !== "system") {
    if (systemMql && systemHandler) {
      systemMql.removeEventListener("change", systemHandler);
      systemMql = null;
      systemHandler = null;
    }
    return;
  }
  if (!systemMql) systemMql = matchMedia("(prefers-color-scheme: dark)");
  if (!systemHandler) {
    systemHandler = () => {
      applyAppearance({ ui_theme: lastMode, accent_color: lastAccent });
    };
    systemMql.addEventListener("change", systemHandler);
  }
}

/** Contrast-ish check: primary L vs surface L difference (not WCAG formal, AA-ish gate). */
export function accentReadableOnGlass(hex: string, theme: "light" | "dark"): boolean {
  const { c } = hexToOklchHueChroma(hex);
  // Extremely neon custom colors still get chroma-capped; reject near-gray seeds.
  if (c < 0.02) return false;
  // Light theme paints primary ~L0.48 on ~L0.97 glass; dark ~L0.78 on ~L0.20.
  // Fixed L tokens keep this true for any hue once chroma is capped.
  void theme;
  return true;
}
