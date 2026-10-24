import { chromium } from "/tmp/shots/node_modules/.pnpm/playwright-core@1.64.1/node_modules/playwright-core/index.mjs";
const EXE = "/root/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome";
const BASE = "http://localhost:5188";
const PAGES = ["home", "download", "instances", "settings"];
const WIDTHS = [560, 900, 1400];

const browser = await chromium.launch({ executablePath: EXE, args: ["--no-sandbox"] });

for (const w of WIDTHS) {
  const page = await browser.newPage({ viewport: { width: w, height: 900 } });
  page.on("pageerror", (e) => console.log("  [pageerror]", e.message.slice(0, 160)));
  for (const nav of PAGES) {
    await page.goto(`${BASE}/?nav=${nav}`, { waitUntil: "networkidle" });
    await page.waitForTimeout(1500);
    const r = await page.evaluate(() => {
      const app = document.querySelector("easymc-app").shadowRoot;
      const content = app.querySelector(".content");
      const pageEl = content.querySelector(".page");
      const child = pageEl.firstElementChild;
      const sr = child?.shadowRoot;
      const cRect = content.getBoundingClientRect();
      const pRect = pageEl.getBoundingClientRect();
      const out = {
        page: child?.tagName.toLowerCase() || "?",
        ovf: content.scrollWidth - content.clientWidth,
        pageGap: `${Math.round(pRect.left - cRect.left)}/${Math.round(cRect.right - pRect.right)}`,
        lefts: [],
        sticky: null,
        loaders: 0,
      };
      if (sr) {
        const blocks = sr.querySelectorAll("h1, .card, .md-tabs, easymc-install-progress, .panel-head, .ver-grid, .loader-grid");
        const seen = new Set();
        blocks.forEach((b) => {
          const x = Math.round(b.getBoundingClientRect().left);
          if (x > 0) seen.add(x);
        });
        out.lefts = [...seen].sort((a, b) => a - b);
        const ip = sr.querySelector("easymc-install-progress");
        if (ip) {
          const st = getComputedStyle(ip);
          out.sticky = `${st.position}/${Math.round(ip.getBoundingClientRect().width)}`;
        }
        out.loaders = sr.querySelectorAll(".loader-ico svg").length;
      }
      return out;
    });
    console.log(
      `w=${String(w).padEnd(5)} ${nav.padEnd(9)} ${r.page.padEnd(22)} ovf=${String(r.ovf).padStart(4)} pageGap=${r.pageGap.padEnd(11)} lefts=[${r.lefts.join(",")}] sticky=${r.sticky} loaderSvg=${r.loaders}`,
    );
  }
  await page.close();
}
await browser.close();
