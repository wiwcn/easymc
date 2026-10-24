import { LitElement, css, html, nothing, svg } from "lit";
import { customElement, state } from "lit/decorators.js";
import {
  createInstance,
  getLogs,
  installLoaderVersion,
  installVersion,
  launchInstance,
  listAccounts,
  listInstancesState,
  listLoaderVersions,
  listVersions,
  listenInstallProgress,
  listenLogs,
  setActiveInstance,
  stopInstance,
  writeLaunchScript,
  openInstanceFolder,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import type { AccountDto, InstanceDto, LoaderId, VersionSummary } from "../types";
import {
  instancePlayerLabel,
  loaderLabel,
  playerVersionLabel,
  versionId,
  versionType,
} from "../types";
import { showToast } from "../toast";

const PLAY = svg`<path d="M8 5.5v13l11-6.5L8 5.5Z" fill="currentColor"/>`;
const MORE = svg`<circle cx="12" cy="5" r="1.6" fill="currentColor"/><circle cx="12" cy="12" r="1.6" fill="currentColor"/><circle cx="12" cy="19" r="1.6" fill="currentColor"/>`;

/** Soft pastel tiles for instance cards — sketch style, not plain letters. */
const TILE_PALETTES = [
  { bg: "oklch(0.90 0.05 155)", fg: "oklch(0.38 0.08 155)" },
  { bg: "oklch(0.91 0.05 70)", fg: "oklch(0.42 0.09 55)" },
  { bg: "oklch(0.90 0.04 235)", fg: "oklch(0.40 0.07 235)" },
  { bg: "oklch(0.90 0.05 186)", fg: "oklch(0.38 0.08 186)" },
  { bg: "oklch(0.91 0.045 310)", fg: "oklch(0.40 0.08 310)" },
];

function tileFor(id: string) {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0;
  return TILE_PALETTES[Math.abs(h) % TILE_PALETTES.length];
}

function tileGlyph(loader: string) {
  const l = (loader || "vanilla").toLowerCase();
  if (l === "fabric" || l === "quilt") {
    return svg`<path d="M7 17V7h2.2l2.3 6.2L13.8 7H16v10h-1.7v-6.4L12.2 17h-1.4L8.7 10.6V17H7Z" fill="currentColor"/>`;
  }
  if (l === "forge" || l === "neoforge") {
    return svg`<path d="M6 16.5 12 5.5l6 11H6Zm3.2-1.8h5.6L12 9.2 9.2 14.7Z" fill="currentColor"/>`;
  }
  return svg`<rect x="6" y="6" width="12" height="12" rx="2.5" fill="currentColor" opacity="0.9"/><path d="M9 9h6v2H9V9Zm0 4h4v2H9v-2Z" fill="var(--md-sys-color-surface-container-lowest)" opacity="0.85"/>`;
}

function formatLastPlayed(iso?: string | null): string {
  if (!iso) return "还没玩过";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "还没玩过";
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const time = d.toLocaleTimeString("zh-CN", { hour: "numeric", minute: "2-digit", hour12: false });
  if (sameDay) return `上次游玩：今天 ${time}`;
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  const isY =
    d.getFullYear() === yesterday.getFullYear() &&
    d.getMonth() === yesterday.getMonth() &&
    d.getDate() === yesterday.getDate();
  if (isY) return `上次游玩：昨天 ${time}`;
  return `上次游玩：${d.toLocaleDateString("zh-CN")} ${time}`;
}

@customElement("easymc-home-page")
export class HomePage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private accounts: AccountDto[] = [];
  @state() private loading = true;
  @state() private busy = false;
  @state() private status = "";
  @state() private progress: InstallProgressDto | null = null;
  @state() private showLogs = false;
  @state() private logs: string[] = [];
  @state() private menuOpenId: string | null = null;
  @state() private featuredMenu = false;
  @state() private manageMenu = false;
  @state() private instancesExpanded = false;
  @state() private showCreate = false;
  @state() private newName = "";
  @state() private newVersion = "1.21.1";
  @state() private includeSnapshots = false;
  @state() private versions: VersionSummary[] = [];
  @state() private versionsLoading = false;
  @state() private versionFilter = "";
  @state() private alsoInstall = true;
  @state() private withAssets = true;
  @state() private newLoader: LoaderId = "vanilla";
  @state() private loaderVersions: { version: string; stable: boolean; intermediary: string }[] = [];
  @state() private newLoaderVersion = "";

  private unlog?: () => void;
  private unprog?: () => void;

  static styles = css`
    :host {
      display: block;
      max-width: 980px;
      position: relative;
    }
    .page-head {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 8px;
      margin-bottom: 12px;
      min-height: 36px;
    }
    .page-head .grow { flex: 1; }
    button.manage {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface-variant);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 7px 14px 7px 16px;
      cursor: pointer;
      font-size: 0.88rem;
      font-weight: 600;
    }
    button.manage:hover {
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
    }
    button.manage svg { width: 16px; height: 16px; }
    button.primary {
      border: none;
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 8px 16px;
      cursor: pointer;
      font-weight: 600;
      font-size: 0.9rem;
    }
    button.ghost {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: transparent;
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 8px 14px;
      cursor: pointer;
      font-size: 0.9rem;
    }
    button.linkish {
      border: none;
      background: transparent;
      color: var(--md-sys-color-primary);
      font-weight: 600;
      font-size: 0.9rem;
      cursor: pointer;
      padding: 6px 4px;
    }

    /* ——— Featured card (home-v1) ——— */
    .featured {
      display: flex;
      flex-direction: column;
      background: var(--md-sys-color-surface-container-lowest);
      border-radius: 20px;
      box-shadow: var(--md-sys-elevation-1);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 45%, transparent);
      overflow: hidden;
      margin-bottom: 22px;
      animation: focal-in var(--easymc-dur-focal) var(--easymc-ease-out) both;
      transition:
        margin var(--easymc-dur-med) var(--easymc-ease-out);
    }
    .featured.compact {
      margin-bottom: 14px;
    }
    @keyframes focal-in {
      from { opacity: 0; transform: translateY(8px); }
      to { opacity: 1; transform: none; }
    }
    .featured-top {
      display: flex;
      gap: 24px;
      padding: 28px 28px 22px;
      align-items: stretch;
      min-height: 196px;
      transition:
        padding var(--easymc-dur-med) var(--easymc-ease-out),
        min-height var(--easymc-dur-med) var(--easymc-ease-out),
        gap var(--easymc-dur-med) var(--easymc-ease-out);
    }
    .featured.compact .featured-top {
      padding: 14px 18px 12px;
      gap: 16px;
      min-height: 0;
      align-items: center;
    }
    .art {
      width: 152px;
      height: 168px;
      border-radius: 20px;
      flex-shrink: 0;
      align-self: center;
      background:
        radial-gradient(circle at 70% 28%, oklch(0.95 0.04 95), transparent 40%),
        linear-gradient(165deg, oklch(0.88 0.05 186), oklch(0.82 0.06 175));
      position: relative;
      overflow: hidden;
      transition:
        width var(--easymc-dur-med) var(--easymc-ease-out),
        height var(--easymc-dur-med) var(--easymc-ease-out);
    }
    .featured.compact .art {
      width: 72px;
      height: 72px;
      border-radius: 14px;
    }
    .art::after {
      content: "";
      position: absolute;
      left: -10%;
      right: -10%;
      bottom: 0;
      height: 42%;
      background: oklch(0.55 0.08 160 / 0.55);
      border-radius: 50% 50% 0 0;
    }
    .art::before {
      content: "";
      position: absolute;
      width: 22px;
      height: 22px;
      border-radius: 50%;
      background: oklch(0.92 0.06 95);
      top: 22%;
      right: 22%;
      box-shadow: 0 0 0 6px oklch(0.92 0.06 95 / 0.25);
    }
    .feat-body {
      flex: 1;
      min-width: 0;
      display: flex;
      flex-direction: column;
      justify-content: center;
    }
    .feat-title {
      margin: 0 0 12px;
      font-size: 1.55rem;
      font-weight: 700;
      letter-spacing: -0.03em;
      color: var(--md-sys-color-on-surface);
      line-height: 1.2;
    }
    .featured.compact .feat-title {
      margin: 0 0 6px;
      font-size: 1.15rem;
    }
    .featured.compact .feat-desc { display: none; }
    .featured.compact .chips { margin-bottom: 0; }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-bottom: 10px;
    }
    .chip {
      font-size: 0.78rem;
      font-weight: 600;
      padding: 4px 12px;
      border-radius: var(--md-sys-shape-corner-full);
      letter-spacing: -0.01em;
    }
    .chip.outline {
      border: 1.5px solid var(--md-sys-color-outline-variant);
      color: var(--md-sys-color-on-surface-variant);
      background: transparent;
    }
    .chip.solid {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border: 1.5px solid transparent;
    }
    .feat-desc {
      margin: 0;
      font-size: 0.92rem;
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.4;
    }
    .featured-bar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex-wrap: wrap;
      padding: 16px 26px;
      border-top: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      background: color-mix(in oklch, var(--md-sys-color-surface-container-low) 55%, transparent);
    }
    .featured.compact .featured-bar {
      padding: 10px 16px;
    }
    .feat-meta {
      font-size: 0.85rem;
      color: var(--md-sys-color-on-surface-variant);
      min-width: 0;
    }
    .feat-actions {
      display: flex;
      align-items: center;
      gap: 8px;
    }
    button.launch {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      border: none;
      /* Soft teal → green gradient — CTA only; no gradient text */
      background: linear-gradient(
        135deg,
        oklch(0.52 0.09 186) 0%,
        oklch(0.50 0.11 165) 55%,
        oklch(0.48 0.12 150) 100%
      );
      color: var(--md-sys-color-on-primary);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 15px 28px;
      font-size: 1.08rem;
      font-weight: 700;
      letter-spacing: -0.01em;
      cursor: pointer;
      box-shadow:
        var(--md-sys-elevation-2),
        0 1px 0 oklch(1 0 0 / 0.18) inset;
      transition:
        filter var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .featured.compact button.launch {
      padding: 10px 20px;
      font-size: 0.95rem;
      gap: 8px;
    }
    button.launch:hover { filter: brightness(1.06); }
    button.launch:active { transform: scale(0.985); }
    button.launch:disabled { opacity: 0.55; cursor: not-allowed; transform: none; }
    button.launch svg { width: 20px; height: 20px; }
    .featured.compact button.launch svg { width: 16px; height: 16px; }
    button.icon-btn {
      width: 40px;
      height: 40px;
      border-radius: 50%;
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface-variant);
      cursor: pointer;
      display: grid;
      place-items: center;
      padding: 0;
    }
    button.icon-btn svg { width: 18px; height: 18px; }
    .menu-wrap { position: relative; }
    .menu-panel {
      position: absolute;
      right: 0;
      top: calc(100% + 6px);
      z-index: 50;
      min-width: 200px;
      background: var(--md-sys-color-surface-container-lowest);
      border-radius: var(--md-sys-shape-corner-medium);
      box-shadow: var(--md-sys-elevation-2);
      padding: 6px;
      display: flex;
      flex-direction: column;
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 50%, transparent);
    }
    .menu-panel button {
      border: none;
      background: transparent;
      text-align: left;
      padding: 10px 12px;
      border-radius: var(--md-sys-shape-corner-small);
      cursor: pointer;
      color: var(--md-sys-color-on-surface);
      font-size: 0.9rem;
    }
    .menu-panel button:hover { background: var(--md-sys-color-surface-container); }

    .status-line {
      font-size: 0.82rem;
      color: var(--md-sys-color-on-surface-variant);
      margin: 0 22px 14px;
    }
    .bar {
      height: 6px;
      border-radius: 999px;
      background: var(--md-sys-color-surface-container-high);
      overflow: hidden;
      margin-top: 6px;
    }
    .bar > i {
      display: block;
      height: 100%;
      background: var(--md-sys-color-primary);
      width: var(--w, 0%);
      transition: width var(--easymc-dur-press) var(--easymc-ease-out);
    }

    /* ——— Instance grid ——— */
    .section-head {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      margin: 4px 0 12px;
    }
    .section-head h2 {
      margin: 0;
      font-size: 1.05rem;
      font-weight: 650;
      letter-spacing: -0.02em;
    }
    .instances-area {
      display: flex;
      flex-direction: column;
      min-height: 0;
    }
    .instances-area.expanded {
      max-height: min(42vh, 360px);
      overflow-y: auto;
      padding-right: 4px;
      margin-right: -4px;
      scrollbar-gutter: stable;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 12px;
    }
    @media (max-width: 860px) {
      .grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }
    @media (max-width: 560px) {
      .grid { grid-template-columns: 1fr; }
      .art { width: 120px; height: 132px; }
      .featured-top { min-height: 0; }
    }
    button.vcard {
      position: relative;
      display: flex;
      flex-direction: row;
      align-items: center;
      gap: 14px;
      text-align: left;
      padding: 14px 16px;
      min-height: 88px;
      border: 1.5px solid transparent;
      border-radius: 18px;
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      cursor: pointer;
      box-shadow: var(--md-sys-elevation-1);
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        border-color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.vcard:hover {
      box-shadow: var(--md-sys-elevation-2);
    }
    button.vcard:active { transform: scale(0.985); }
    button.vcard.selected {
      border-color: var(--md-sys-color-primary);
      box-shadow: var(--md-sys-elevation-1), 0 0 0 1px var(--md-sys-color-primary);
    }
    .vcard-body {
      flex: 1;
      min-width: 0;
      display: flex;
      flex-direction: column;
      gap: 4px;
      padding-right: 28px;
    }
    .tile {
      width: 48px;
      height: 48px;
      border-radius: 14px;
      display: grid;
      place-items: center;
      flex-shrink: 0;
    }
    .tile svg { width: 26px; height: 26px; }
    .vcard .title {
      font-size: 0.98rem;
      font-weight: 650;
      letter-spacing: -0.02em;
      line-height: 1.25;
    }
    .vcard .meta {
      font-size: 0.78rem;
      color: var(--md-sys-color-on-surface-variant);
    }
    .vcard-foot {
      display: flex;
      align-items: center;
      gap: 8px;
      width: 100%;
      flex-wrap: wrap;
      margin-top: 2px;
    }
    .vchip {
      font-size: 0.72rem;
      font-weight: 600;
      padding: 3px 9px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
    }
    .ready {
      display: inline-flex;
      align-items: center;
      gap: 5px;
      font-size: 0.75rem;
      font-weight: 600;
      color: var(--md-sys-color-success);
    }
    .ready.not {
      color: var(--md-sys-color-on-surface-variant);
    }
    .ready i {
      width: 7px;
      height: 7px;
      border-radius: 50%;
      background: currentColor;
    }
    .vcard .more {
      position: absolute;
      top: 10px;
      right: 8px;
      width: 32px;
      height: 32px;
      border: none;
      background: transparent;
      color: var(--md-sys-color-on-surface-variant);
      border-radius: 50%;
      cursor: pointer;
      display: grid;
      place-items: center;
      padding: 0;
    }
    .vcard .more:hover {
      background: var(--md-sys-color-surface-container);
    }
    .vcard .more svg { width: 16px; height: 16px; }

    .muted { color: var(--md-sys-color-on-surface-variant); font-size: 0.85rem; }
    .empty {
      text-align: center;
      padding: 48px 20px 32px;
      display: flex;
      flex-direction: column;
      align-items: center;
    }
    .empty h1 {
      font-size: 1.25rem;
      font-weight: 650;
      letter-spacing: -0.02em;
      margin: 0 0 8px;
    }
    .empty .sub {
      color: var(--md-sys-color-on-surface-variant);
      margin: 0 0 20px;
      max-width: 28em;
      line-height: 1.5;
    }
    .empty .row { justify-content: center; }
    .row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
    .card {
      margin-top: 14px;
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
    }
    .card h2 { margin: 0 0 6px; font-size: 0.9rem; }
    pre.logs {
      max-height: 180px;
      overflow: auto;
      font-size: 0.75rem;
      background: var(--md-sys-color-surface-container);
      padding: 10px;
      border-radius: 8px;
    }
    .create {
      margin-top: 14px;
      padding: 16px;
      border-radius: 18px;
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
    }
    input, select {
      border: 1px solid var(--md-sys-color-outline-variant);
      border-radius: var(--md-sys-shape-corner-small);
      padding: 8px 10px;
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
    }
    @media (prefers-reduced-motion: reduce) {
      .featured { animation: none; }
      .featured, .featured-top, .art, button.launch { transition: none; }
      button.vcard:active, button.launch:active { transform: none; }
      .bar > i { transition: none; }
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.boot();
    void listenLogs((l) => {
      this.logs = [...this.logs.slice(-200), l];
    }).then((u) => (this.unlog = u));
    void listenInstallProgress((p) => (this.progress = p)).then((u) => (this.unprog = u));
    window.addEventListener("click", this.onDocClick);
  }
  disconnectedCallback() {
    this.unlog?.();
    this.unprog?.();
    window.removeEventListener("click", this.onDocClick);
    super.disconnectedCallback();
  }
  private onDocClick = () => {
    if (this.menuOpenId || this.featuredMenu || this.manageMenu) {
      this.menuOpenId = null;
      this.featuredMenu = false;
      this.manageMenu = false;
    }
  };

  private get current(): InstanceDto | null {
    return this.instances.find((i) => i.id === this.activeId) ?? this.instances[0] ?? null;
  }
  private get account(): AccountDto | null {
    const cur = this.current;
    if (cur?.account_id) {
      return this.accounts.find((a) => a.id === cur.account_id) ?? this.accounts[0] ?? null;
    }
    return this.accounts[0] ?? null;
  }

  private async boot() {
    this.loading = true;
    try {
      const [st, acc] = await Promise.all([listInstancesState(), listAccounts()]);
      this.instances = st.instances;
      this.activeId = st.active_id;
      this.accounts = acc;
      this.logs = await getLogs();
    } finally {
      this.loading = false;
    }
  }

  private async onSelect(id: string) {
    try {
      const st = await setActiveInstance(id);
      this.instances = st.instances;
      this.activeId = st.active_id;
    } catch {
      this.activeId = id;
    }
  }

  private async onPlay() {
    const f = this.current;
    if (!f) return;
    this.busy = true;
    this.status = "";
    this.progress = null;
    try {
      const ver = versionId(f.version);
      if (!f.version_ready) {
        this.status = "正在准备游戏文件…";
        if (f.loader === "vanilla" || !f.loader) {
          await installVersion(ver, true);
        } else {
          await installLoaderVersion(ver, f.loader, f.loader_version ?? null, true);
        }
      }
      this.status = "正在启动游戏…";
      const msg = await launchInstance(f.id);
      this.status = msg;
      showToast("游戏已启动");
      await this.boot();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
      this.progress = null;
    }
  }

  private go(nav: string) {
    this.dispatchEvent(new CustomEvent("nav-change", { detail: nav, bubbles: true, composed: true }));
  }

  private async openCreate() {
    this.showCreate = true;
    this.versionsLoading = true;
    try {
      this.versions = await listVersions(this.includeSnapshots);
    } finally {
      this.versionsLoading = false;
    }
  }

  private async refreshLoaderVersions() {
    if (this.newLoader === "vanilla") {
      this.loaderVersions = [];
      this.newLoaderVersion = "";
      return;
    }
    this.loaderVersions = await listLoaderVersions(this.newLoader, this.newVersion);
    this.newLoaderVersion =
      this.loaderVersions.find((v) => v.stable)?.version ?? this.loaderVersions[0]?.version ?? "";
  }

  private async onCreate() {
    if (!this.newVersion.trim()) {
      showToast("请选择游戏版本");
      return;
    }
    this.busy = true;
    try {
      const name = this.newName.trim() || playerVersionLabel(this.newVersion, this.newLoader);
      const inst = await createInstance(
        name,
        this.newVersion,
        this.newLoader,
        this.newLoader === "vanilla" ? null : this.newLoaderVersion || null,
      );
      if (this.alsoInstall) {
        if (this.newLoader === "vanilla") await installVersion(this.newVersion, this.withAssets);
        else
          await installLoaderVersion(
            this.newVersion,
            this.newLoader,
            this.newLoaderVersion || null,
            this.withAssets,
          );
      }
      await this.onSelect(inst.id);
      this.showCreate = false;
      showToast(`已添加「${instancePlayerLabel(inst)}」`);
      await this.boot();
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.busy = false;
    }
  }

  private displayName(i: InstanceDto): string {
    const label = instancePlayerLabel(i);
    const custom = (i.name || "").trim();
    if (custom && custom !== label && !custom.startsWith(versionId(i.version))) return custom;
    return label;
  }

  private renderFeatured(f: InstanceDto) {
    const ver = versionId(f.version);
    const loader = loaderLabel(f.loader);
    const title = this.displayName(f);
    const desc =
      (f.description || "").trim() ||
      (f.version_ready ? "选好就能开玩，点启动进入游戏。" : "还没装好游戏文件，点启动会先安装再进入。");
    const ready = !!f.version_ready;
    const playLabel = ready ? "启动游戏" : "安装并启动";
    const acc = this.account;
    const pct = this.progress?.percent;
    const metaBits = [
      formatLastPlayed(f.last_played),
      acc ? `以 ${acc.username} 进入` : null,
    ].filter(Boolean);

    const compact = this.instancesExpanded;
    return html`
      <section class="featured ${compact ? "compact" : ""}" aria-label="当前版本">
        <div class="featured-top">
          <div class="art" aria-hidden="true"></div>
          <div class="feat-body">
            <h1 class="feat-title">${title}</h1>
            <div class="chips">
              <span class="chip outline">${ver}</span>
              <span class="chip solid">${loader}</span>
              ${ready
                ? html`<span class="chip outline">已就绪</span>`
                : html`<span class="chip outline">未安装</span>`}
            </div>
            <p class="feat-desc">${desc}</p>
          </div>
        </div>
        <div class="featured-bar">
          <div class="feat-meta">${metaBits.join(" · ")}</div>
          <div class="feat-actions">
            <button class="launch" type="button" ?disabled=${this.busy} @click=${() => this.onPlay()}>
              <svg viewBox="0 0 24 24">${PLAY}</svg>
              ${this.busy ? "请稍候…" : playLabel}
            </button>
            <div class="menu-wrap">
              <button
                class="icon-btn"
                type="button"
                title="更多操作"
                aria-haspopup="menu"
                @click=${(e: Event) => {
                  e.stopPropagation();
                  this.featuredMenu = !this.featuredMenu;
                  this.menuOpenId = null;
                }}
              >
                <svg viewBox="0 0 24 24">${MORE}</svg>
              </button>
              ${this.featuredMenu ? this.renderMenu(f, true) : nothing}
            </div>
          </div>
        </div>
        ${this.status || this.progress
          ? html`<div class="status-line">
              ${this.progress
                ? html`${this.progress.stage_label} · ${this.progress.detail}
                    <div class="bar"><i style=${`--w:${pct ?? 0}%`}></i></div>`
                : this.status}
            </div>`
          : nothing}
      </section>
    `;
  }

  private renderMenu(f: InstanceDto, featured: boolean) {
    return html`<div
      class="menu-panel"
      role="menu"
      style=${featured ? "" : "bottom:auto;top:calc(100% + 4px);"}
      @click=${(e: Event) => e.stopPropagation()}
    >
      <button
        type="button"
        @click=${() => {
          this.featuredMenu = false;
          this.menuOpenId = null;
          this.go("instances");
        }}
      >
        管理此版本
      </button>
      <button
        type="button"
        @click=${async () => {
          this.featuredMenu = false;
          this.menuOpenId = null;
          try {
            showToast(await openInstanceFolder(f.id, "game"));
          } catch (e) {
            showToast(e instanceof Error ? e.message : String(e));
          }
        }}
      >
        打开游戏文件夹
      </button>
      <button
        type="button"
        ?disabled=${this.busy}
        @click=${async () => {
          this.featuredMenu = false;
          this.menuOpenId = null;
          try {
            const r = await writeLaunchScript(f.id);
            showToast(r.message_zh);
          } catch (e) {
            showToast(e instanceof Error ? e.message : String(e));
          }
        }}
      >
        生成启动脚本
      </button>
      <button
        type="button"
        @click=${() => {
          this.featuredMenu = false;
          this.menuOpenId = null;
          this.showLogs = !this.showLogs;
        }}
      >
        ${this.showLogs ? "隐藏日志" : "查看日志"}
      </button>
      <button
        type="button"
        @click=${() => {
          this.featuredMenu = false;
          this.menuOpenId = null;
          void stopInstance();
          showToast("已请求停止");
        }}
      >
        停止游戏
      </button>
    </div>`;
  }

  private renderCard(i: InstanceDto) {
    const label = instancePlayerLabel(i);
    const title = this.displayName(i);
    const selected = i.id === (this.activeId ?? this.current?.id);
    const tile = tileFor(i.id);
    const ready = !!i.version_ready;
    const tint = `color-mix(in oklch, ${tile.bg} 70%, var(--md-sys-color-surface-container-lowest))`;
    return html`
      <button
        class="vcard ${selected ? "selected" : ""}"
        type="button"
        aria-pressed=${selected ? "true" : "false"}
        style=${`background:${tint}`}
        @click=${() => this.onSelect(i.id)}
      >
        <span
          class="more"
          role="button"
          tabindex="0"
          title="更多"
          @click=${(e: Event) => {
            e.stopPropagation();
            this.menuOpenId = this.menuOpenId === i.id ? null : i.id;
            this.featuredMenu = false;
          }}
          @keydown=${(e: KeyboardEvent) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              e.stopPropagation();
              this.menuOpenId = this.menuOpenId === i.id ? null : i.id;
            }
          }}
        >
          <svg viewBox="0 0 24 24">${MORE}</svg>
        </span>
        ${this.menuOpenId === i.id
          ? html`<div class="menu-wrap" style="position:absolute;top:40px;right:8px;z-index:20">
              ${this.renderMenu(i, false)}
            </div>`
          : nothing}
        <span class="tile" style=${`background:${tile.bg};color:${tile.fg}`} aria-hidden="true">
          <svg viewBox="0 0 24 24">${tileGlyph(i.loader)}</svg>
        </span>
        <span class="vcard-body">
          <span class="title">${title}</span>
          <span class="meta">${label}</span>
          <div class="vcard-foot">
            <span class="ready ${ready ? "" : "not"}"><i></i>${ready ? "已就绪" : "未安装"}</span>
          </div>
        </span>
      </button>
    `;
  }

  render() {
    if (this.loading) return html`<p class="muted">加载中…</p>`;
    const f = this.current;
    if (!f) {
      return html`
        <div class="empty">
          <h1>咦？似乎什么都没有…</h1>
          <p class="sub">还没有可玩的版本。加一个，或者去下载中心逛逛？</p>
          <div class="row">
            <button class="primary" type="button" @click=${() => this.openCreate()}>添加版本</button>
            <button class="ghost" type="button" @click=${() => this.go("download")}>去下载中心</button>
          </div>
          ${this.showCreate ? this.renderCreate() : nothing}
        </div>
      `;
    }
    const others = this.instances;
    const visible = this.instancesExpanded ? others : others.slice(0, 3);
    const canExpand = others.length > 3;
    return html`
      <div class="page-head">
        <span class="grow"></span>
        <div class="menu-wrap">
          <button
            class="manage"
            type="button"
            aria-haspopup="menu"
            aria-expanded=${this.manageMenu ? "true" : "false"}
            title="管理"
            @click=${(e: Event) => {
              e.stopPropagation();
              this.manageMenu = !this.manageMenu;
              this.featuredMenu = false;
              this.menuOpenId = null;
            }}
          >
            管理
            <svg viewBox="0 0 24 24">${MORE}</svg>
          </button>
          ${this.manageMenu
            ? html`<div
                class="menu-panel"
                role="menu"
                @click=${(e: Event) => e.stopPropagation()}
              >
                <button
                  type="button"
                  @click=${() => {
                    this.manageMenu = false;
                    void this.openCreate();
                  }}
                >
                  添加版本
                </button>
                <button
                  type="button"
                  @click=${() => {
                    this.manageMenu = false;
                    this.go("download");
                  }}
                >
                  去下载中心
                </button>
                <button
                  type="button"
                  @click=${() => {
                    this.manageMenu = false;
                    this.go("instances");
                  }}
                >
                  管理版本
                </button>
              </div>`
            : nothing}
        </div>
      </div>
      ${this.renderFeatured(f)}
      <div class="section-head">
        <h2>你的版本</h2>
        ${canExpand
          ? html`<button
              class="linkish"
              type="button"
              @click=${() => {
                this.instancesExpanded = !this.instancesExpanded;
              }}
            >
              ${this.instancesExpanded ? "收起" : "显示全部"}
            </button>`
          : html`<button class="linkish" type="button" @click=${() => this.go("instances")}>
              查看全部
            </button>`}
      </div>
      <div class="instances-area ${this.instancesExpanded ? "expanded" : ""}">
        <div class="grid" role="listbox" aria-label="版本列表">
          ${visible.map((i) => this.renderCard(i))}
        </div>
      </div>
      ${this.showLogs
        ? html`<div class="card">
            <h2>启动日志</h2>
            <pre class="logs">${this.logs.join("\n") || "（还没动静）"}</pre>
          </div>`
        : nothing}
      ${this.showCreate ? this.renderCreate() : nothing}
    `;
  }

  private renderCreate() {
    const filtered = this.versions.filter((v) => {
      if (!this.versionFilter.trim()) return true;
      return versionId(v as never).includes?.(this.versionFilter) || v.id.includes(this.versionFilter);
    });
    return html`
      <div class="create">
        <h2 style="margin-top:0">添加版本</h2>
        <div class="row" style="margin-bottom:8px">
          <input
            placeholder="自定义名称（可选，卡片仍显示版本·加载器）"
            .value=${this.newName}
            @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)}
          />
          <select
            .value=${this.newLoader}
            @change=${(e: Event) => {
              this.newLoader = (e.target as HTMLSelectElement).value as LoaderId;
              void this.refreshLoaderVersions();
            }}
          >
            <option value="vanilla">原版</option>
            <option value="fabric">Fabric</option>
            <option value="quilt">Quilt</option>
            <option value="forge">Forge</option>
            <option value="neoforge">NeoForge</option>
          </select>
        </div>
        <div class="row" style="margin-bottom:8px">
          <input
            placeholder="筛选版本…"
            .value=${this.versionFilter}
            @input=${(e: Event) => (this.versionFilter = (e.target as HTMLInputElement).value)}
          />
          <label class="muted"
            ><input
              type="checkbox"
              .checked=${this.includeSnapshots}
              @change=${async (e: Event) => {
                this.includeSnapshots = (e.target as HTMLInputElement).checked;
                this.versions = await listVersions(this.includeSnapshots);
              }}
            />
            含快照</label
          >
        </div>
        <select
          style="width:100%;margin-bottom:8px"
          .value=${this.newVersion}
          @change=${(e: Event) => {
            this.newVersion = (e.target as HTMLSelectElement).value;
            void this.refreshLoaderVersions();
          }}
        >
          ${this.versionsLoading
            ? html`<option>加载中…</option>`
            : filtered
                .slice(0, 80)
                .map((v) => html`<option value=${v.id}>${v.id} · ${versionType(v)}</option>`)}
        </select>
        ${this.newLoader !== "vanilla"
          ? html`<select
              style="width:100%;margin-bottom:8px"
              .value=${this.newLoaderVersion}
              @change=${(e: Event) =>
                (this.newLoaderVersion = (e.target as HTMLSelectElement).value)}
            >
              ${this.loaderVersions.map(
                (v) =>
                  html`<option value=${v.version}>
                    ${v.version}${v.stable ? "（稳定）" : ""}
                  </option>`,
              )}
            </select>`
          : nothing}
        <label class="muted"
          ><input
            type="checkbox"
            .checked=${this.alsoInstall}
            @change=${(e: Event) => (this.alsoInstall = (e.target as HTMLInputElement).checked)}
          />
          添加后立刻安装</label
        >
        <div class="row" style="margin-top:10px">
          <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.onCreate()}>
            添加
          </button>
          <button class="ghost" type="button" @click=${() => (this.showCreate = false)}>取消</button>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-home-page": HomePage;
  }
}
