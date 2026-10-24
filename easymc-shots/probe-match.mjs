import { chromium } from "/tmp/shots/node_modules/.pnpm/playwright-core@1.64.1/node_modules/playwright-core/index.mjs";
const EXE = "/root/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome";
const BASE = "http://localhost:5188";
const browser = await chromium.launch({ executablePath: EXE, args: ["--no-sandbox"] });
const page = await browser.newPage({ viewport: { width: 1100, height: 900 } });
page.on("pageerror", (e) => console.log("[pageerror]", e.message.slice(0, 200)));

const ready = () =>
  page.waitForFunction(
    () => document.querySelector("easymc-app")?.shadowRoot?.querySelector(".content .page")?.firstElementChild?.shadowRoot,
    null,
    { timeout: 10000 },
  );

// --- home: which blocks sit at a non-first left edge?
await page.goto(`${BASE}/?nav=home`, { waitUntil: "networkidle" });
await ready();
await page.waitForTimeout(800);
const home = await page.evaluate(() => {
  const sr = document.querySelector("easymc-app").shadowRoot.querySelector(".content .page").firstElementChild.shadowRoot;
  const ip = sr.querySelector("easymc-install-progress");
  const out = { ip: ip ? getComputedStyle(ip).position : null, matches: [] };
  sr.querySelectorAll("h1, .card, .md-tabs, easymc-install-progress, .panel-head, .ver-grid, .loader-grid").forEach((b) => {
    const rc = b.getBoundingClientRect();
    out.matches.push(`${b.tagName.toLowerCase()}.${(typeof b.className === "string" ? b.className : "").slice(0, 24)} l=${Math.round(rc.left)} w=${Math.round(rc.width)}`);
  });
  return out;
});
console.log("HOME banner position =", home.ip);
home.matches.forEach((m) => console.log("   ", m));

// --- download: walk to loader step, check loader icon SVGs
await page.goto(`${BASE}/?nav=download`, { waitUntil: "networkidle" });
await ready();
await page.waitForTimeout(1000);
const dl = await page.evaluate(() => {
  const sr = document.querySelector("easymc-app").shadowRoot.querySelector(".content .page").firstElementChild.shadowRoot;
  const ip = sr.querySelector("easymc-install-progress");
  const verCards = sr.querySelectorAll("button.ver-card");
  const first = verCards[0];
  if (first) first.click();
  return { ip: ip ? getComputedStyle(ip).position : null, verCards: verCards.length };
});
await page.waitForTimeout(1200);
const dl2 = await page.evaluate(() => {
  const sr = document.querySelector("easymc-app").shadowRoot.querySelector(".content .page").firstElementChild.shadowRoot;
  const loaders = [...sr.querySelectorAll("button.loader-card")].map((b) => b.textContent.trim().replace(/\s+/g, " ").slice(0, 20));
  const svgs = sr.querySelectorAll(".loader-ico svg").length;
  const icons = [...sr.querySelectorAll(".loader-ico")].map((i) => (i.className || "").replace("loader-ico", "").trim());
  const lvCards = sr.querySelectorAll("button.ver-card.lv").length;
  const verGrid = sr.querySelector(".ver-cards");
  return { loaders, svgs, icons, lvCards, hasVerCards: !!verGrid };
});
console.log("\nDOWNLOAD after picking version:", JSON.stringify(dl));
console.log("   loaderCards=", dl2.loaders);
console.log("   loaderIconClasses=", dl2.icons, "svgCount=", dl2.svgs);
console.log("   loaderVersionCards(.lv)=", dl2.lvCards, "ver-cards grid=", dl2.hasVerCards);
await browser.close();
