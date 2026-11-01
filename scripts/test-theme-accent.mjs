/**
 * Theme / accent unit checks.
 */
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

async function loadTheme() {
  const esbuild = await import("esbuild");
  const outfile = path.join(os.tmpdir(), `easymc-theme-test-${process.pid}.mjs`);
  await esbuild.build({
    entryPoints: [path.join(__dirname, "../src/lib/theme.ts")],
    bundle: true,
    outfile,
    format: "esm",
    platform: "neutral",
    target: "es2022",
  });
  try {
    return await import(pathToFileURL(outfile).href + `?t=${Date.now()}`);
  } finally {
    try { fs.unlinkSync(outfile); } catch { /* ignore */ }
  }
}

const props = new Map();
globalThis.document = {
  documentElement: {
    setAttribute() {},
    style: {
      setProperty(k, v) { props.set(k, v); },
      colorScheme: "",
    },
  },
};
globalThis.matchMedia = () => ({
  matches: false,
  addEventListener() {},
  removeEventListener() {},
});

const theme = await loadTheme();

assert.equal(theme.normalizeUiTheme("DARK"), "dark");
assert.equal(theme.normalizeUiTheme("nope"), "light");
assert.equal(theme.normalizeAccentHex("#0d6"), "#00DD66");
assert.equal(theme.normalizeAccentHex("0D6E67"), "#0D6E67");
assert.equal(theme.normalizeAccentHex("zzz"), null);

const teal = theme.hexToOklchHueChroma("#0D6E67");
assert.ok(Math.abs(teal.h - 186.5) < 12, `teal hue ~186 got ${teal.h}`);
assert.ok(teal.c > 0.04, `teal chroma got ${teal.c}`);

for (const p of theme.ACCENT_PRESETS) {
  assert.ok(theme.accentReadableOnGlass(p.hex, "light"), p.id);
  assert.ok(theme.accentReadableOnGlass(p.hex, "dark"), p.id);
}

// Spot-check a few hues don't collapse chroma (glass wash guard)
for (const hex of ["#FF0000", "#00FF00", "#0000FF", "#FF00FF", "#FFD700"]) {
  const applied = theme.applyAppearance({ ui_theme: "light", accent_color: hex });
  assert.ok(applied.chroma <= 0.11, hex);
  assert.ok(applied.chroma >= 0.06, hex);
  assert.ok(theme.accentReadableOnGlass(hex, "light"), hex);
}

const applied = theme.applyAppearance({ ui_theme: "dark", accent_color: "#C2185B" });
assert.equal(applied.resolved, "dark");
assert.equal(applied.accent, "#C2185B");
assert.ok(props.has("--easymc-accent-hue"));
assert.ok(Number(props.get("--easymc-accent-chroma")) <= 0.11);

const def = theme.applyAppearance({ ui_theme: "light", accent_color: null });
assert.equal(def.accent, theme.DEFAULT_ACCENT_HEX);

console.log("test-theme-accent: ok", {
  presets: theme.ACCENT_PRESETS.length,
  roseHue: Number(applied.hue.toFixed(1)),
  chroma: applied.chroma,
});
