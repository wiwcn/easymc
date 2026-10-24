import { chromium } from "/tmp/shots/node_modules/.pnpm/playwright-core@1.64.1/node_modules/playwright-core/index.mjs";
const EXE = "/root/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome";
const BASE = "http://localhost:5188";
const browser = await chromium.launch({ executablePath: EXE, args: ["--no-sandbox"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
page.on("pageerror", (e) => console.log("[pageerror]", e.message.slice(0, 200)));
await page.goto(`${BASE}/?nav=settings`, { waitUntil: "networkidle" });
await page
  .waitForFunction(
    () => document.querySelector("easymc-app")?.shadowRoot?.querySelector(".content .page")?.firstElementChild?.shadowRoot,
    null,
    { timeout: 10000 },
  )
  .catch(() => {});
await page.waitForTimeout(1200);
const r = await page.evaluate(() => {
  const sr = document.querySelector("easymc-app").shadowRoot.querySelector(".content .page").firstElementChild.shadowRoot;
  return {
    cards: [...sr.querySelectorAll(".card h2")].map((h) => h.textContent.trim()),
    tabs: [...sr.querySelectorAll(".md-tabs button")].map((b) => b.textContent.trim()),
  };
});
console.log("settings tabs:", r.tabs.join(" | "));
console.log("settings cards:", r.cards.join(" | "));
await browser.close();
