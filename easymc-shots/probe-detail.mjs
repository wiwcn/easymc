import { chromium } from "/tmp/shots/node_modules/.pnpm/playwright-core@1.64.1/node_modules/playwright-core/index.mjs";
const EXE = "/root/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome";
const BASE = "http://localhost:5188";
const browser = await chromium.launch({ executablePath: EXE, args: ["--no-sandbox"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
page.on("pageerror", (e) => console.log("[pageerror]", e.message.slice(0, 200)));

for (const nav of ["home", "download", "instances"]) {
  await page.goto(`${BASE}/?nav=${nav}`, { waitUntil: "networkidle" });
  await page.waitForTimeout(1500);
  const r = await page.evaluate(() => {
    const app = document.querySelector("easymc-app").shadowRoot;
    const pageEl = app.querySelector(".content .page");
    const host = pageEl.firstElementChild;
    const sr = host.shadowRoot;
    const rows = [];
    const walk = (root) => {
      const kids = root.children ? [...root.children] : [];
      for (const k of kids) {
        if (k.tagName === "STYLE" || k.tagName === "SCRIPT") continue;
        const rc = k.getBoundingClientRect();
        if (rc.width === 0 && rc.height === 0) continue;
        rows.push({
          tag: k.tagName.toLowerCase(),
          cls: (k.className && typeof k.className === "string" ? k.className : "").slice(0, 28),
          l: Math.round(rc.left), t: Math.round(rc.top), w: Math.round(rc.width), h: Math.round(rc.height),
        });
      }
    };
    walk(sr); // top-level blocks of the page component
    const ip = sr.querySelector("easymc-install-progress");
    let ipInfo = null;
    if (ip) {
      const st = getComputedStyle(ip);
      const rc = ip.getBoundingClientRect();
      ipInfo = { pos: st.position, top: st.top, z: st.zIndex, l: Math.round(rc.left), t: Math.round(rc.top), w: Math.round(rc.width), h: Math.round(rc.height), minH: st.minHeight, flex: st.flex };
    }
    return { host: host.tagName.toLowerCase(), rows: rows.slice(0, 14), ipInfo };
  });
  console.log(`\n=== ${nav} (${r.host}) ip=${JSON.stringify(r.ipInfo)}`);
  for (const x of r.rows) console.log(`   ${x.tag.padEnd(22)} ${String(x.cls).padEnd(28)} l=${String(x.l).padStart(4)} t=${String(x.t).padStart(4)} ${x.w}x${x.h}`);
}
await browser.close();
