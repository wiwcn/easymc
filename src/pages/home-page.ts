import { LitElement, css, html, nothing, svg } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
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
import { friendlyError } from "../lib/friendly-error";
import "../components/install-progress";
import "../components/empty-state";

const PLAY = svg`<path d="M8 5.5v13l11-6.5L8 5.5Z" fill="currentColor"/>`;
const MORE = svg`<circle cx="12" cy="5" r="1.6" fill="currentColor"/><circle cx="12" cy="12" r="1.6" fill="currentColor"/><circle cx="12" cy="19" r="1.6" fill="currentColor"/>`;

/** Soft pastel tiles for instance cards — sketch style, not plain letters. */
const TILE_PALETTES = [
  { bg: "var(--easymc-tile-0-bg)", fg: "var(--easymc-tile-0-fg)" },
  { bg: "var(--easymc-tile-1-bg)", fg: "var(--easymc-tile-1-fg)" },
  { bg: "var(--easymc-tile-2-bg)", fg: "var(--easymc-tile-2-fg)" },
  { bg: "var(--easymc-tile-3-bg)", fg: "var(--easymc-tile-3-fg)" },
  { bg: "var(--easymc-tile-4-bg)", fg: "var(--easymc-tile-4-fg)" },
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
  @state() private launchFailed = false;
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

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    :host {
      display: flex;
      flex-direction: column;
      flex: 1 0 auto;
      min-height: 100%;
      width: 100%;
      position: relative;
      box-sizing: border-box;
    }
    /* Column width now comes from the shell (.page > * in app-shell.ts), so
       every page shares one measure instead of its own ad-hoc max-width. */
    .page-head {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: var(--easymc-space-2);
      margin-bottom: var(--easymc-space-3);
      min-height: 36px;
    }
    .page-head .grow { flex: 1; }
    button.manage {
      display: inline-flex;
      align-items: center;
      gap: var(--easymc-space-2);
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface-variant);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 7px 14px 7px 16px;
      cursor: pointer;
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        color var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.manage:hover {
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
    }
    button.manage:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }
    button.manage svg { width: 16px; height: 16px; }
    /* primary / ghost / launch → controlStyles (CMP-1) */
    button.linkish {
      border: none;
      background: transparent;
      color: var(--md-sys-color-primary);
      font-weight: var(--easymc-weight-medium);
      font-size: var(--easymc-type-label);
      cursor: pointer;
      padding: 6px 4px;
      border-radius: var(--md-sys-shape-corner-small);
    }
    button.linkish:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }

    /* ——— Featured card (home-v1) ——— */
    .featured {
      display: flex;
      flex-direction: column;
      flex-shrink: 0; /* never collapse under「显示全部」grid */
      background: var(--md-sys-color-surface-container-lowest);
      border-radius: var(--md-sys-shape-corner-extra-large);
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elevation, no border */
      overflow: visible; /* menus must not clip; art still clips via .art */
      margin-bottom: var(--easymc-space-5);
      transition:
        margin var(--easymc-dur-med) var(--easymc-ease-out);
      position: relative;
      z-index: 2;
    }
    .featured.compact {
      margin-bottom: var(--easymc-space-4);
    }
    .featured-top {
      display: flex;
      gap: var(--easymc-space-5);
      padding: var(--easymc-space-6) var(--easymc-space-6) var(--easymc-space-5);
      align-items: stretch;
      min-height: 228px;
      transition:
        padding var(--easymc-dur-med) var(--easymc-ease-out),
        min-height var(--easymc-dur-med) var(--easymc-ease-out),
        gap var(--easymc-dur-med) var(--easymc-ease-out);
    }
    .featured.compact .featured-top {
      padding: var(--easymc-space-4) var(--easymc-space-4) var(--easymc-space-3);
      gap: var(--easymc-space-4);
      min-height: 96px; /* keep title + art visible when list expands */
      align-items: center;
    }
    .art {
      width: 176px;
      height: 196px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      flex-shrink: 0;
      align-self: center;
      background:
        radial-gradient(circle at 70% 28%, var(--easymc-art-glow), transparent 40%),
        linear-gradient(165deg, var(--easymc-art-base-from), var(--easymc-art-base-to));
      position: relative;
      overflow: hidden;
      transition:
        width var(--easymc-dur-med) var(--easymc-ease-out),
        height var(--easymc-dur-med) var(--easymc-ease-out);
    }
    .featured.compact .art {
      width: 72px;
      height: 72px;
      border-radius: var(--md-sys-shape-corner-medium);
    }
    .art::after {
      content: "";
      position: absolute;
      left: -10%;
      right: -10%;
      bottom: 0;
      height: 42%;
      background: var(--easymc-art-hill);
      border-radius: 50% 50% 0 0;
    }
    .art::before {
      content: "";
      position: absolute;
      width: 22px;
      height: 22px;
      border-radius: 50%;
      background: var(--easymc-art-sun);
      top: 22%;
      right: 22%;
      box-shadow: 0 0 0 6px var(--easymc-art-sun-halo);
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
      font-size: var(--easymc-type-display);
      font-weight: var(--easymc-weight-bold);
      letter-spacing: -0.03em;
      color: var(--md-sys-color-on-surface);
      line-height: 1.2;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      max-width: 100%;
    }
    .featured.compact .feat-title {
      margin: 0 0 4px;
      font-size: var(--easymc-type-title);
    }
    .featured.compact .feat-desc {
      display: -webkit-box;
      -webkit-line-clamp: 1;
      -webkit-box-orient: vertical;
      overflow: hidden;
      font-size: var(--easymc-type-meta);
      margin: 0;
    }
    .featured.compact .chips { margin-bottom: 4px; }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: var(--easymc-space-2);
      margin-bottom: var(--easymc-space-3);
    }
    .chip {
      display: inline-flex;
      align-items: center;
      min-height: 24px; /* A11Y-4 decorative chip floor */
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      padding: var(--easymc-space-1) var(--easymc-space-3);
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
    .chip.container {
      background: var(--md-sys-color-tertiary-container);
      color: var(--md-sys-color-on-tertiary-container);
      border: 1.5px solid transparent;
      gap: 5px;
    }
    .chip.container::before {
      content: "";
      width: 6px;
      height: 6px;
      border-radius: 50%;
      background: currentColor;
      opacity: 0.85;
      flex-shrink: 0;
    }
    .feat-desc {
      margin: 0;
      font-size: var(--easymc-type-body);
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.4;
    }
    .featured-bar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: var(--easymc-space-3);
      flex-wrap: wrap;
      padding: var(--easymc-space-5) var(--easymc-space-6);
      border-top: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      background: color-mix(in oklch, var(--md-sys-color-surface-container-low) 55%, transparent);
    }
    .featured.compact .featured-bar {
      padding: var(--easymc-space-3) var(--easymc-space-4);
    }
    .feat-meta {
      font-size: var(--easymc-type-label);
      color: var(--md-sys-color-on-surface-variant);
      min-width: 0;
    }
    .feat-actions {
      display: flex;
      align-items: center;
      gap: var(--easymc-space-2);
    }
    /* launch base → controlStyles; compact size override */
    .featured.compact button.launch {
      padding: 10px 20px;
      font-size: var(--easymc-type-body);
      gap: 8px;
    }
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
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        color var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.icon-btn:hover {
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface);
    }
    button.icon-btn:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }
    button.icon-btn svg { width: 18px; height: 18px; }
    .menu-wrap { position: relative; }
    .menu-panel {
      position: absolute;
      right: 0;
      top: calc(100% + 6px);
      z-index: 80;
      min-width: 200px;
      background: var(--md-sys-color-surface-container-lowest);
      border-radius: var(--md-sys-shape-corner-medium);
      box-shadow: var(--md-sys-elevation-2); /* CF-1: elevation, no border */
      padding: 6px;
      display: flex;
      flex-direction: column;
      /* MI-8: grow from the trigger's edge, exit faster than it entered. */
      opacity: 0;
      visibility: hidden;
      pointer-events: none;
      transform: translateY(-6px) scale(0.96);
      transform-origin: top right;
      transition:
        opacity var(--easymc-dur-fast) var(--easymc-ease-out),
        transform var(--easymc-dur-fast) var(--easymc-ease-out),
        visibility 0s linear var(--easymc-dur-fast);
    }
    /* Featured bar sits low — open upward so the panel is not clipped by the page. */
    .menu-panel.open-up {
      top: auto;
      bottom: calc(100% + 6px);
      transform: translateY(6px) scale(0.96);
      transform-origin: bottom right;
    }
    .menu-panel.open {
      opacity: 1;
      visibility: visible;
      pointer-events: auto;
      transform: none;
      transition:
        opacity var(--easymc-dur-med) var(--easymc-ease-out),
        transform var(--easymc-dur-med) var(--easymc-ease-out),
        visibility 0s;
    }
    .menu-panel button {
      border: none;
      background: transparent;
      text-align: left;
      padding: 10px 12px;
      border-radius: var(--md-sys-shape-corner-small);
      cursor: pointer;
      color: var(--md-sys-color-on-surface);
      font-size: var(--easymc-type-label);
    }
    .menu-panel button:hover { background: var(--md-sys-color-surface-container); }

    .status-line {
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      margin: 0 var(--easymc-space-5) var(--easymc-space-4);
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--easymc-space-2);
    }
    .status-agent {
      border: none;
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 6px 12px;
      font: inherit;
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      cursor: pointer;
    }
    .status-agent:hover { filter: brightness(1.05); }
    .status-agent:focus-visible { box-shadow: var(--easymc-focus-ring); }
    /* ——— Instance grid ——— */
    .section-head {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      margin: var(--easymc-space-2) 0 var(--easymc-space-3);
      gap: var(--easymc-space-3);
    }
    /* section-head h2 → pageChromeStyles */
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
    /* Fluid instance grid: tracks add/remove themselves with the column width,
       so no breakpoint is needed and wide windows get more cards per row. */
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(272px, 100%), 1fr));
      gap: var(--easymc-space-3);
    }
    /* Only the hero art needs an explicit narrow rule: everything else in the
       featured card is already fluid. */
    @container easymc-page (max-width: 560px) {
      .art { width: 128px; height: 144px; }
      .featured-top { min-height: 0; }
    }
    button.vcard {
      position: relative;
      display: flex;
      flex-direction: row;
      align-items: center;
      gap: var(--easymc-space-4);
      text-align: left;
      padding: var(--easymc-space-4);
      min-height: 88px;
      border: 1.5px solid transparent;
      border-radius: var(--md-sys-shape-corner-large);
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
    button.vcard:focus-visible {
      box-shadow: var(--easymc-focus-ring), var(--md-sys-elevation-1);
    }
    button.vcard.selected {
      /* quieter (P2): container tint + border, no elevation (CF-1 XOR) */
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      box-shadow: none;
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
      border-radius: var(--md-sys-shape-corner-medium);
      display: grid;
      place-items: center;
      flex-shrink: 0;
    }
    .tile svg { width: 26px; height: 26px; }
    .vcard .title {
      font-size: var(--easymc-type-body);
      font-weight: var(--easymc-weight-medium);
      letter-spacing: -0.02em;
      line-height: 1.25;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      max-width: 100%;
    }
    .vcard .meta {
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .vcard-foot {
      display: flex;
      align-items: center;
      gap: var(--easymc-space-2);
      width: 100%;
      flex-wrap: wrap;
      margin-top: 2px;
    }
    .vchip {
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      padding: 3px 9px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-surface-container);
      color: var(--md-sys-color-on-surface-variant);
    }
    .vchip.container {
      background: var(--md-sys-color-tertiary-container);
      color: var(--md-sys-color-on-tertiary-container);
    }
    .ready {
      display: inline-flex;
      align-items: center;
      gap: 5px;
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
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
    /* Soft empty / not-installed: keep elev, mute the tile so ready cards lead */
    button.vcard.not-ready {
      background: var(--md-sys-color-surface-container-low);
      opacity: 0.92;
    }
    button.vcard.not-ready .tile {
      filter: grayscale(0.35);
      opacity: 0.85;
    }
    button.vcard.not-ready .title {
      color: var(--md-sys-color-on-surface-variant);
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
    .vcard .more:focus-visible {
      box-shadow: var(--easymc-focus-ring);
    }
    .vcard .more svg { width: 16px; height: 16px; }


    .home-skel { display: flex; flex-direction: column; gap: var(--easymc-space-5); padding-top: var(--easymc-space-2); }
    .featured-skel { height: 220px; border-radius: var(--md-sys-shape-corner-extra-large); }
    /* Mirrors .grid so the skeleton reserves the same tracks as real content. */
    .skel-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(272px, 100%), 1fr));
      gap: var(--easymc-space-3);
    }
    .vcard-skel { height: 88px; border-radius: var(--md-sys-shape-corner-large); }
    .muted { color: var(--md-sys-color-on-surface-variant); font-size: var(--easymc-type-meta); }
    .empty-wrap {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 100%;
      align-self: stretch;
      width: 100%;
    }
    easymc-empty-state button.primary,
    easymc-empty-state button.ghost {
      /* inherit page button tokens via light-DOM in host shadow */
    }
    .row { display: flex; gap: var(--easymc-space-2); flex-wrap: wrap; align-items: center; }
    .card {
      margin-top: 14px;
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
    }
    .card h2 { margin: 0 0 6px; font-size: var(--easymc-type-label); }
    pre.logs {
      max-height: 180px;
      overflow: auto;
      font-size: var(--easymc-type-meta);
      background: var(--md-sys-color-surface-container);
      padding: 10px;
      border-radius: var(--md-sys-shape-corner-small);
    }
    .create {
      margin-top: 14px;
      padding: 16px;
      border-radius: var(--md-sys-shape-corner-large);
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
      .featured, .featured-top, .art, button.launch { transition: none; }
      button.vcard:active, button.launch:active { transform: none; }
      .menu-panel, .menu-panel.open { transition: none; transform: none; }
    }
  `,
  ];

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
  private revealed = false;

  protected updated() {
    // C-5: fade content in once the first load resolves (skeleton → content).
    if (!this.loading && !this.revealed) {
      this.revealed = true;
      this.classList.add("md-loaded");
    }
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
    this.launchFailed = false;
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
      this.launchFailed = false;
      showToast("游戏已启动");
      await this.boot();
    } catch (e) {
      this.status = friendlyError(e, "启动没成功，可让助手看看");
      this.launchFailed = true;
      showToast("启动失败，可让助手看看");
    } finally {
      this.busy = false;
      this.progress = null;
    }
  }

  private goAgentDiagnose() {
    sessionStorage.setItem("easymc-agent-autodiagnose", "1");
    this.go("agent");
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
      showToast(friendlyError(e));
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
              ${f.container
                ? html`<span class="chip container" title="模组与配置已隔离，互不干扰">容器</span>`
                : nothing}
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
              ${this.renderMenu(f, true, this.featuredMenu)}
            </div>
          </div>
        </div>
        ${this.status && !this.progress
          ? html`<div class="status-line">
              <span>${this.status}</span>
              ${this.launchFailed
                ? html`<button class="status-agent" type="button" @click=${() => this.goAgentDiagnose()}>
                    让助手看看
                  </button>`
                : nothing}
            </div>`
          : nothing}
      </section>
    `;
  }

  private renderMenu(f: InstanceDto, featured: boolean, open: boolean) {
    return html`<div
      class="menu-panel ${open ? "open" : ""} ${featured ? "open-up" : ""}"
      role="menu"
      aria-hidden=${open ? "false" : "true"}
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
            showToast(friendlyError(e));
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
            showToast(friendlyError(e));
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
        class="vcard ${selected ? "selected" : ""} ${ready ? "" : "not-ready"}"
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
        <div class="menu-wrap" style="position:absolute;top:40px;right:8px;z-index:20">
          ${this.renderMenu(i, false, this.menuOpenId === i.id)}
        </div>
        <span class="tile" style=${`background:${tile.bg};color:${tile.fg}`} aria-hidden="true">
          <svg viewBox="0 0 24 24">${tileGlyph(i.loader)}</svg>
        </span>
        <span class="vcard-body">
          <span class="title">${title}</span>
          <span class="meta">${label}</span>
          <div class="vcard-foot">
            <span class="ready ${ready ? "" : "not"}"><i></i>${ready ? "已就绪" : "未安装"}</span>
            ${i.container ? html`<span class="vchip container" title="模组与配置已隔离，互不干扰">容器</span>` : nothing}
          </div>
        </span>
      </button>
    `;
  }

  render() {
    if (this.loading) {
      return html`
        <div class="home-skel" aria-busy="true" aria-label="正在加载版本">
          <div class="skel skel-block featured-skel"></div>
          <div class="skel-grid">
            <div class="skel skel-block vcard-skel"></div>
            <div class="skel skel-block vcard-skel"></div>
            <div class="skel skel-block vcard-skel"></div>
          </div>
        </div>
      `;
    }
    const f = this.current;
    if (!f) {
      return html`
        <div class="empty-wrap">
          <easymc-empty-state
            variant="page"
            title="咦？似乎什么都没有…"
            description="还没有可玩的版本。加一个，或者去下载中心逛逛？"
          >
            <button class="primary" type="button" @click=${() => this.go("download")}>去下载中心</button>
            <button class="ghost" type="button" @click=${() => this.openCreate()}>添加版本</button>
          </easymc-empty-state>
          ${this.showCreate ? this.renderCreate() : nothing}
        </div>
      `;
    }
    const others = this.instances;
    const visible = this.instancesExpanded ? others : others.slice(0, 3);
    const canExpand = others.length > 3;
    return html`
      <easymc-install-progress .progress=${this.progress}></easymc-install-progress>
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
          <div
            class="menu-panel ${this.manageMenu ? "open" : ""}"
            role="menu"
            aria-hidden=${this.manageMenu ? "false" : "true"}
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
          </div>
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
