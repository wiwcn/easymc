import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  createInstance,
  curseforgeConfigured,
  installContent,
  installLoaderVersion,
  installMod,
  installModpack,
  installVersion,
  listInstancesState,
  listLoaderVersions,
  listModVersions,
  listVersions,
  listenInstallProgress,
  listLocalContent,
  searchContent,
  searchMods,
  setActiveInstance,
  setCurseforgeKey,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import type {
  CurseForgeStatusDto,
  InstanceDto,
  LoaderId,
  ModProjectDto,
  ModSourceId,
  ModVersionDto,
  ProjectTypeId,
  VersionSummary,
} from "../types";
import { loaderLabel, versionType } from "../types";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import {
  marketEmptyTitle,
  marketInstallCta,
  marketNeedsInstance,
  type MarketTab,
} from "../lib/download-ux";
import { syncIndicator } from "../lib/indicator";
import { loaderIcon } from "../loader-icons";
import "../components/install-progress";
import "../components/empty-state";

type DlTab = "vanilla" | "mod" | "modpack" | "resourcepack" | "shader" | "datapack";

/** Loader builds are numerous (Forge can be 100+); preview this many as cards. */
const LOADER_VER_PREVIEW = 24;

@customElement("easymc-download-page")
export class DownloadPage extends LitElement {
  @state() private tab: DlTab = "vanilla";
  @state() private source: ModSourceId = "modrinth";
  @state() private query = "";
  @state() private results: ModProjectDto[] = [];
  @state() private loading = false;
  @state() private error = "";
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private cfKeyDraft = "";
  @state() private progress: InstallProgressDto | null = null;
  @state() private installingId: string | null = null;
  /** Project open in the page-level version-pick view (null = market grid). */
  @state() private pickProject: ModProjectDto | null = null;
  @state() private pickVersionId: string | null = null;
  @state() private versions: ModVersionDto[] = [];
  @state() private versionsLoading = false;
  @state() private worldPick = "";
  @state() private worlds: string[] = [];

  // vanilla tab
  @state() private mcVersions: VersionSummary[] = [];
  @state() private snapshots = false;
  @state() private filter = "";
  @state() private pick = "1.21.1";
  @state() private loader: LoaderId = "vanilla";
  @state() private loaderVers: { version: string; stable: boolean }[] = [];
  @state() private loaderVer = "";
  @state() private loaderVerExpanded = false;
  @state() private loaderVersLoading = false;
  @state() private newName = "";
  @state() private busy = false;
  @state() private vanillaLoading = false;
  /** Vanilla install wizard: pick MC version → pick loader. */
  @state() private vanillaStep: "version" | "loader" = "version";

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
      box-sizing: border-box;
      /* Column width comes from the shell (.page > * in app-shell.ts), so every
         page shares one measure instead of its own ad-hoc max-width. */
    }
    /* h1 / .sub → pageChromeStyles */
    /* .tabs → controlStyles .md-tabs (MC-2, shared vocabulary + sliding indicator) */
    .search-row {
      display: flex; gap: var(--easymc-space-3); flex-wrap: wrap; align-items: center; margin-bottom: var(--easymc-space-3);
    }
    .search {
      flex: 1; min-width: min(200px, 100%); display: flex; align-items: center; gap: var(--easymc-space-2);
      padding: 10px 16px; border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-surface-container-lowest);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 70%, transparent);
      /* CF-1: border only, no elevation */
    }
    .search input {
      flex: 1; border: none; background: transparent; padding: 0; outline: none;
      border-radius: 0; /* CMP-5: pill lives on .search shell */
      font-size: var(--easymc-type-body); color: var(--md-sys-color-on-surface);
    }
    .search svg { width: 18px; height: 18px; color: var(--md-sys-color-on-surface-variant); flex-shrink: 0; }
    .sources { display: flex; gap: var(--easymc-space-2); flex-wrap: wrap; align-items: center; }
    .sources .lbl { font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant); margin-right: 2px; }
    /* source filters → controlStyles .md-chip (MC-4) */
    .dot {
      width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0;
    }
    .dot.mr { background: var(--easymc-brand-modrinth); }
    .dot.cf { background: var(--easymc-brand-curseforge); }
    .bar {
      display: flex; gap: var(--easymc-space-2); flex-wrap: wrap; align-items: center; margin-bottom: var(--easymc-space-3);
    }
    /* CMP-5: form fields use small radius; search / tabs / chips stay pill */
    input, select {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-small);
      padding: 8px 14px; font-size: var(--easymc-type-label);
    }
    button {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full); padding: 8px 14px; font-size: var(--easymc-type-label);
    }
    /* primary / ghost → controlStyles (CMP-1); card install uses primary */
    .market-chrome {
      display: flex; flex-direction: column; gap: var(--easymc-space-3);
      margin-bottom: var(--easymc-space-3);
    }
    .search .clear {
      border: none; background: transparent; color: var(--md-sys-color-on-surface-variant);
      cursor: pointer; padding: 4px 6px; border-radius: var(--md-sys-shape-corner-full);
      font-size: var(--easymc-type-meta); font-weight: var(--easymc-weight-medium);
      flex-shrink: 0;
    }
    .search .clear:hover { color: var(--md-sys-color-on-surface); background: var(--md-sys-color-surface-container); }
    .results-meta {
      display: flex; align-items: baseline; justify-content: space-between; gap: var(--easymc-space-2);
      margin: 0 0 var(--easymc-space-3); font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .need-inst {
      display: flex; flex-wrap: wrap; align-items: center; gap: var(--easymc-space-3);
      padding: 12px 16px; margin-bottom: var(--easymc-space-3);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
      font-size: var(--easymc-type-label); line-height: 1.4;
    }
    .need-inst .grow { flex: 1; min-width: 160px; }
    .loader-cta-hint {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant);
      margin: var(--easymc-space-2) 0 0;
    }
    .loader-skel {
      display: grid; grid-template-columns: repeat(auto-fill, minmax(min(140px, 100%), 1fr));
      gap: var(--easymc-space-2); margin-top: var(--easymc-space-3);
    }
    .loader-skel .skel { height: 44px; border-radius: var(--md-sys-shape-corner-large); }
    .market-body {
      flex: 1 1 auto; display: flex; flex-direction: column; min-height: 220px;
    }
    .market-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr));
      gap: var(--easymc-grid-gap);
      align-items: stretch;
    }
    @container easymc-page (max-width: 720px) {
      .market-grid,
      .market-skel {
        grid-template-columns: repeat(
          auto-fill,
          minmax(min(var(--easymc-grid-min-dense-narrow), 100%), 1fr)
        );
      }
    }
    .card {
      container-type: inline-size;
      display: flex; flex-direction: column; gap: var(--easymc-space-3);
      padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
      border-radius: var(--easymc-card-radius);
      background: var(--easymc-glass-surface);
      backdrop-filter: blur(var(--easymc-glass-blur));
      -webkit-backdrop-filter: blur(var(--easymc-glass-blur));
      box-shadow: var(--md-sys-elevation-1); /* CF-1: elev, no border */
      align-items: stretch; min-height: 0; height: 100%;
      box-sizing: border-box;
      transition: background var(--easymc-dur-press) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .card:hover {
      background: color-mix(in oklch, var(--md-sys-color-surface-container) 82%, transparent);
      box-shadow: var(--md-sys-elevation-2);
    }
    .card-top { display: flex; gap: var(--easymc-space-3); align-items: flex-start; flex: 0 0 auto; }
    .card img, .card .ph {
      width: 48px; height: 48px; border-radius: var(--md-sys-shape-corner-large); object-fit: cover;
      background: color-mix(in oklch, var(--md-sys-color-primary) 6%, var(--md-sys-color-surface-container));
      flex-shrink: 0;
    }
    .card .body { flex: 1; min-width: 0; }
    .card .name {
      font-weight: var(--easymc-weight-medium); font-size: var(--easymc-type-body); letter-spacing: -0.01em;
      line-height: var(--easymc-leading-body, 1.35);
      display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
      overflow-wrap: break-word; word-break: normal;
    }
    .card .desc {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant); margin-top: 2px;
      line-height: 1.4;
      display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
      overflow-wrap: break-word; word-break: normal;
    }
    .tags { display: flex; flex-wrap: wrap; gap: 6px; margin-top: var(--easymc-space-2); }
    .tag {
      font-size: var(--easymc-type-meta); font-weight: var(--easymc-weight-medium); padding: 2px 8px; border-radius: var(--md-sys-shape-corner-full);
      background: color-mix(in oklch, var(--md-sys-color-secondary-container) 65%, var(--md-sys-color-surface-container-lowest));
      color: var(--md-sys-color-on-secondary-container);
    }
    .card-foot {
      display: flex; flex-wrap: nowrap; align-items: center; justify-content: flex-end;
      gap: var(--easymc-space-2); margin-top: auto; padding-top: var(--easymc-space-1);
      width: 100%;
    }
    .dl-stat {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant);
      font-variant-numeric: tabular-nums; margin-right: auto; min-width: 0;
    }
    .card-foot button.primary {
      padding: 6px 14px;
      font-size: var(--easymc-type-label);
      white-space: nowrap;
      flex: 0 0 auto;
    }
    @media (prefers-reduced-transparency: reduce) {
      .card, .pick-panel {
        background: var(--md-sys-color-surface-container-lowest);
        backdrop-filter: none;
        -webkit-backdrop-filter: none;
      }
      .card:hover {
        background: var(--md-sys-color-surface-container);
      }
    }
    /* .muted → pageChromeStyles */
    .market-skel {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr));
      gap: var(--easymc-space-3);
      margin-bottom: var(--easymc-space-3);
    }
    .card-skel { height: 148px; border-radius: var(--md-sys-shape-corner-large); }
    .vanilla-skel { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr)); gap: var(--easymc-grid-gap); }
    .ver-skel { height: 72px; border-radius: var(--md-sys-shape-corner-large); }

    /* Page-level version pick (leaves the card grid) */
    .pick-panel {
      display: flex; flex-direction: column; gap: var(--easymc-space-4);
      padding: var(--easymc-space-5);
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--easymc-glass-surface);
      backdrop-filter: blur(var(--easymc-glass-blur));
      -webkit-backdrop-filter: blur(var(--easymc-glass-blur));
      box-shadow: var(--md-sys-elevation-1);
      min-height: 280px;
      animation: easymc-pick-in var(--easymc-dur-med) var(--easymc-ease-out) both;
    }
    @keyframes easymc-pick-in {
      from { opacity: 0; transform: translateY(8px); }
      to { opacity: 1; transform: translateY(0); }
    }
    @media (prefers-reduced-motion: reduce) {
      .pick-panel { animation: none; }
    }
    .pick-hero {
      display: flex; gap: var(--easymc-space-4); align-items: flex-start;
    }
    .pick-hero img, .pick-hero .ph {
      width: 64px; height: 64px; border-radius: var(--md-sys-shape-corner-large); object-fit: cover;
      background: color-mix(in oklch, var(--md-sys-color-primary) 6%, var(--md-sys-color-surface-container));
      flex-shrink: 0;
    }
    .pick-hero .body { flex: 1; min-width: 0; }
    .pick-hero .name {
      font-size: var(--easymc-type-title); font-weight: var(--easymc-weight-medium);
      letter-spacing: -0.01em; margin: 0 0 4px;
    }
    .pick-hero .desc {
      margin: 0; font-size: var(--easymc-type-body); color: var(--md-sys-color-on-surface-variant);
      line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden;
      overflow-wrap: break-word; word-break: normal;
    }
    .pick-meta {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant); margin-top: var(--easymc-space-2);
    }
    .pick-versions {
      display: flex; flex-direction: column; gap: var(--easymc-space-2);
      max-height: min(420px, 52vh); overflow: auto; scrollbar-gutter: stable;
    }
    button.pick-ver {
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      display: flex; flex-direction: column; gap: 2px; align-items: stretch;
      padding: var(--easymc-space-3) var(--easymc-space-4);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      box-shadow: none; /* CF-1: outline inside glass panel */
      transition: background var(--easymc-dur-press) var(--easymc-ease-out),
        border-color var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.pick-ver:hover { background: var(--md-sys-color-surface-container); }
    button.pick-ver:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    button.pick-ver.on {
      background: color-mix(in oklch, var(--md-sys-color-primary) 8%, var(--md-sys-color-surface));
      border-color: var(--md-sys-color-primary);
    }
    button.pick-ver .vid {
      font-weight: var(--easymc-weight-medium); font-size: var(--easymc-type-body);
      font-variant-numeric: tabular-nums;
    }
    button.pick-ver .vmeta {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant);
      overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    }
    .pick-foot {
      display: flex; flex-wrap: wrap; align-items: center; justify-content: flex-end;
      gap: var(--easymc-space-2); margin-top: auto; padding-top: var(--easymc-space-2);
      border-top: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 50%, transparent);
    }
    .pick-foot .hint {
      margin-right: auto; font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant);
    }

    .results-slot {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 220px;
    }
    .vanilla-body {
      display: flex;
      flex-direction: column;
      flex: 1 1 auto;
      min-height: 240px;
    }
    .panel {
      padding: var(--easymc-space-4); border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
    }
    .field { margin-bottom: 10px; display: flex; flex-direction: column; gap: var(--easymc-space-1); max-width: 480px; }
    .field label { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); }

    .toolbar {
      display: flex; gap: var(--easymc-space-3); flex-wrap: wrap; align-items: center; margin-bottom: var(--easymc-space-4);
    }
    .toolbar .grow { flex: 1; min-width: 160px; }
    .crumb {
      display: flex; align-items: center; gap: var(--easymc-space-2); flex-wrap: wrap;
      font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); margin-bottom: var(--easymc-space-3);
    }
    .crumb strong { color: var(--md-sys-color-on-surface); font-weight: var(--easymc-weight-medium); }
    .crumb button {
      border: none; background: transparent; color: var(--md-sys-color-primary);
      cursor: pointer; font: inherit; font-weight: var(--easymc-weight-medium); padding: 0;
    }
    .ver-grid, .loader-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr));
      gap: var(--easymc-grid-gap);
    }
    @container easymc-page (max-width: 720px) {
      .ver-grid, .loader-grid, .vanilla-skel {
        grid-template-columns: repeat(
          auto-fill,
          minmax(min(var(--easymc-grid-min-dense-narrow), 100%), 1fr)
        );
      }
    }
    button.ver-card, button.loader-card {
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      display: flex; gap: var(--easymc-space-3); align-items: flex-start;
      padding: var(--easymc-space-3) var(--easymc-space-3); border-radius: var(--easymc-card-radius);
      min-height: 72px;
      /* Lighter Operate surface: outline XOR elev (CF-1) — idle uses outline */
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      box-shadow: none;
      transition: box-shadow var(--easymc-dur-press) var(--easymc-ease-out),
        background var(--easymc-dur-press) var(--easymc-ease-out),
        border-color var(--easymc-dur-press) var(--easymc-ease-out),
        outline-color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.ver-card:hover, button.loader-card:hover {
      background: var(--md-sys-color-surface-container);
      border-color: color-mix(in oklch, var(--md-sys-color-outline-variant) 80%, transparent);
      box-shadow: none;
    }
    button.ver-card:active, button.loader-card:active { transform: scale(0.985); }
    button.ver-card:focus-visible, button.loader-card:focus-visible {
      outline: none;
      box-shadow: var(--easymc-focus-ring);
    }
    button.ver-card { align-items: center; } /* version tiles stay vertically centered */
    button.loader-card { align-items: flex-start; } /* multi-line loader copy */
    button.ver-card .vid {
      font-weight: var(--easymc-weight-bold); font-size: var(--easymc-type-title); letter-spacing: -0.02em;
    }
    button.ver-card .meta {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant); margin-top: 2px;
    }
    .badge-inst {
      margin-left: auto; font-size: var(--easymc-type-meta); font-weight: var(--easymc-weight-medium);
      padding: 3px 8px; border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container); flex-shrink: 0;
    }
    .badge-stable {
      font-size: var(--easymc-type-meta); font-weight: var(--easymc-weight-medium);
      padding: 2px 7px; border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container); flex-shrink: 0;
    }
    /* Selected card: the pick needs to be legible at a glance, so the fill (not
       just the border) changes. */
    button.ver-card.on, button.loader-card.on {
      /* CF-1 XOR: selected uses fill + outline, drop border */
      background: var(--md-sys-color-primary-container);
      border-color: transparent;
      outline: 2px solid var(--md-sys-color-primary);
      outline-offset: 0;
      box-shadow: none;
    }
    button.ver-card.on .vid, button.ver-card.on .meta { color: var(--md-sys-color-on-primary-container); }
    .panel-head {
      display: flex; align-items: baseline; justify-content: space-between;
      gap: var(--easymc-space-2); margin-bottom: var(--easymc-space-3);
      font-size: var(--easymc-type-label); font-weight: var(--easymc-weight-medium);
    }
    /* Loader builds as cards: scrolls instead of stretching the page when a
       loader ships hundreds of builds. */
    .ver-cards {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(140px, 100%), 1fr));
      gap: var(--easymc-space-2);
      max-height: 268px; overflow-y: auto;
      scrollbar-gutter: stable; padding-right: 4px; margin-right: -4px;
    }
    button.ver-card.lv {
      padding: 10px 12px; gap: var(--easymc-space-2); justify-content: space-between;
    }
    button.ver-card.lv .vid {
      font-size: var(--easymc-type-label); font-weight: var(--easymc-weight-medium); letter-spacing: 0;
      font-variant-numeric: tabular-nums; overflow: hidden; text-overflow: ellipsis;
    }
    /* Official loader marks: monochrome paths tinted per loader, sitting on a
       matching wash so the tile still reads as a coloured chip. */
    .loader-ico {
      width: 48px; height: 48px; border-radius: var(--md-sys-shape-corner-large); flex-shrink: 0;
      display: grid; place-items: center;
      background: color-mix(in oklch, currentColor 14%, transparent);
      border: 1px solid color-mix(in oklch, currentColor 28%, transparent);
      transition: background var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .loader-ico svg { width: 26px; height: 26px; display: block; }
    .loader-ico.vanilla { color: oklch(0.55 0.12 148); }
    .loader-ico.fabric { color: oklch(0.62 0.13 78); }
    .loader-ico.quilt { color: oklch(0.6 0.14 305); }
    .loader-ico.forge { color: oklch(0.48 0.03 68); }
    .loader-ico.neoforge { color: oklch(0.6 0.14 38); }
    button.loader-card:hover .loader-ico { background: color-mix(in oklch, currentColor 20%, transparent); }
    button.loader-card .lname { font-weight: var(--easymc-weight-bold); font-size: var(--easymc-type-body); }
    button.loader-card .ldesc {
      font-size: var(--easymc-type-meta); color: var(--md-sys-color-on-surface-variant); margin-top: 2px; line-height: 1.35;
      display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
      max-width: 100%;
    }
    .loader-panel {
      padding: var(--easymc-space-4); border-radius: var(--md-sys-shape-corner-large); margin-top: var(--easymc-space-4);
      background: var(--md-sys-color-surface-container-low);
    }
    .loader-panel .row { display: flex; gap: var(--easymc-space-2); flex-wrap: wrap; align-items: center; margin-top: var(--easymc-space-3); }
    button.back {
      border: none; background: transparent; color: var(--md-sys-color-primary);
      cursor: pointer; font: inherit; font-weight: var(--easymc-weight-medium); padding: 8px 4px;
      border-radius: var(--md-sys-shape-corner-small);
    }
    button.back:focus-visible, .crumb button:focus-visible, .search .clear:focus-visible {
      outline: none; box-shadow: var(--easymc-focus-ring);
    }
  `,
  ];

  connectedCallback() {
    super.connectedCallback();
    const saved = sessionStorage.getItem("easymc-download-tab") as DlTab | null;
    if (saved) {
      this.tab = saved;
      sessionStorage.removeItem("easymc-download-tab");
    }
    const q = sessionStorage.getItem("easymc-download-query");
    if (q) {
      this.query = q;
      sessionStorage.removeItem("easymc-download-query");
      if (!saved) this.tab = "mod";
    }
    void this.bootstrap();
    void listenInstallProgress((p) => (this.progress = p)).then((u) => (this.unprog = u));
  }
  disconnectedCallback() {
    this.unprog?.();
    super.disconnectedCallback();
  }

  private revealed = false;

  protected updated() {
    syncIndicator(this.renderRoot);
    // C-5: fade content in once the first load resolves (skeleton → content).
    if (!this.loading && !this.revealed) {
      this.revealed = true;
      this.classList.add("md-loaded");
    }
  }

  private get active(): InstanceDto | null {
    return this.instances.find((i) => i.id === this.activeId) ?? this.instances[0] ?? null;
  }

  private async bootstrap() {
    const [st, cf] = await Promise.all([listInstancesState(), curseforgeConfigured()]);
    this.instances = st.instances;
    this.activeId = st.active_id;
    this.cf = cf;
    if (this.tab === "vanilla") await this.loadVanilla();
    else await this.onSearch(); // empty query → popular/hot
  }

  private async loadVanilla() {
    this.vanillaLoading = true;
    try {
      this.mcVersions = await listVersions(this.snapshots);
      if (!this.mcVersions.some((v) => v.id === this.pick) && this.mcVersions[0]) {
        this.pick = this.mcVersions[0].id;
      }
    } finally {
      this.vanillaLoading = false;
    }
  }

  private async setTab(t: DlTab) {
    this.tab = t;
    if (t === "vanilla") this.vanillaStep = "version";
    this.results = [];
    this.error = "";
    this.pickProject = null;
    this.pickVersionId = null;
    if (t === "vanilla") await this.loadVanilla();
    else await this.onSearch(); // empty query → popular/hot
  }

  private projectType(): ProjectTypeId {
    if (this.tab === "mod") return "mod";
    if (this.tab === "modpack") return "modpack";
    if (this.tab === "resourcepack") return "resourcepack";
    if (this.tab === "shader") return "shader";
    return "datapack";
  }

  private async onSearch() {
    if (this.tab === "vanilla") return;
    this.loading = true;
    this.error = "";
    try {
      if (this.source === "curseforge" && !this.cf?.configured) {
        this.error = "浏览 CurseForge 需要先在设置里粘贴密钥（高级）。";
        this.results = [];
        return;
      }
      const game = this.active ? String(this.active.version) : null;
      const loader = this.tab === "mod" ? this.active?.loader : null;
      this.results =
        this.tab === "mod"
          ? await searchMods(this.source, this.query, game, loader)
          : await searchContent(this.source, this.projectType(), this.query, game, loader);
    } catch (e) {
      this.error = friendlyError(e, "模组列表暂时加载不了");
      this.results = [];
    } finally {
      this.loading = false;
    }
  }

  private closePick() {
    this.pickProject = null;
    this.pickVersionId = null;
    this.versions = [];
  }

  private async openPick(p: ModProjectDto) {
    const needsInst = marketNeedsInstance(this.tab as MarketTab);
    if (needsInst && !this.active) {
      showToast("请先添加一个游戏版本");
      return;
    }
    this.pickProject = p;
    this.pickVersionId = null;
    this.versionsLoading = true;
    this.versions = [];
    try {
      const game = this.active ? String(this.active.version) : null;
      this.versions = await listModVersions(
        this.source,
        p.id,
        game,
        this.tab === "mod" ? this.active?.loader : null,
      );
      if (this.versions[0]) this.pickVersionId = this.versions[0].id;
    } catch (e) {
      showToast(friendlyError(e));
      this.versions = [];
    } finally {
      this.versionsLoading = false;
    }
  }

  private async onInstall(p: ModProjectDto, versionId?: string | null) {
    this.installingId = p.id;
    this.progress = null;
    let ok = false;
    try {
      if (this.tab === "modpack") {
        const r = await installModpack(this.source, p.id, { versionId });
        showToast(r.message_zh);
        const st = await listInstancesState();
        this.instances = st.instances;
        this.activeId = r.instance_id;
        await setActiveInstance(r.instance_id);
        ok = true;
      } else if (this.tab === "mod") {
        if (!this.active) {
          showToast("请先在启动页选好要玩的版本");
          return;
        }
        const r = await installMod(this.source, p.id, versionId, this.active.id);
        showToast(r.skipped ? `已有 ${r.filename}` : `已装入 ${r.project_name}`);
        ok = true;
      } else {
        if (!this.active) {
          showToast("请先在启动页选好要玩的版本");
          return;
        }
        if (this.tab === "datapack" && this.worlds.length === 0) {
          this.worlds = (await listLocalContent(this.active.id, "saves")).map((w) => w.name);
        }
        const r = await installContent(this.source, this.projectType(), p.id, {
          versionId,
          instanceId: this.active.id,
          world: this.tab === "datapack" ? this.worldPick || null : null,
        });
        showToast(r.skipped ? `已有 ${r.filename}` : `已下载 ${r.project_name}`);
        ok = true;
      }
    } catch (e) {
      showToast(friendlyError(e, "安装没成功，请稍后在桌面版重试"));
    } finally {
      this.installingId = null;
    }
    if (ok && this.pickProject?.id === p.id) this.closePick();
  }

  render() {
    const tabs: { id: DlTab; label: string }[] = [
      { id: "vanilla", label: "原版" },
      { id: "mod", label: "模组" },
      { id: "modpack", label: "整合包" },
      { id: "resourcepack", label: "资源包" },
      { id: "shader", label: "光影" },
      { id: "datapack", label: "数据包" },
    ];
    const sub =
      this.tab === "vanilla"
        ? this.vanillaStep === "version"
          ? "选择游戏版本"
          : `为 ${this.pick} 选择加载器`
        : this.tab === "modpack"
          ? "一键装好整套模组包（会新建一个版本）"
          : this.active
            ? `装进当前版本：${this.active.name}`
            : "先在启动页选一个版本，或直接装整合包";
    return html`
      <p class="sub">${sub}</p>
      <div class="md-tabs" role="tablist" aria-label="下载分类">
        <span class="md-tab-ind" aria-hidden="true"></span>
        ${tabs.map((t) => html`<button class=${this.tab === t.id ? "on" : ""} type="button" role="tab" aria-selected=${this.tab === t.id} @click=${() => this.setTab(t.id)}>${t.label}</button>`)}
      </div>
      <easymc-install-progress .progress=${this.progress}></easymc-install-progress>
      ${this.tab === "vanilla" ? this.renderVanilla() : this.renderMarket()}
    `;
  }

  private renderVanilla() {
    const filtered = this.mcVersions.filter((v) => !this.filter || v.id.includes(this.filter));
    if (this.vanillaStep === "loader") {
      const loaders: { id: LoaderId; name: string; desc: string }[] = [
        { id: "vanilla", name: "原版", desc: "纯净 Minecraft，不加模组加载器" },
        { id: "fabric", name: "Fabric", desc: "轻量模组加载器，更新快" },
        { id: "quilt", name: "Quilt", desc: "Fabric 兼容分支" },
        { id: "forge", name: "Forge", desc: "经典模组加载器，大型模组包常用" },
        { id: "neoforge", name: "NeoForge", desc: "Forge 后续，适合新版本" },
      ];
      const shown = this.loaderVerExpanded ? this.loaderVers : this.loaderVers.slice(0, LOADER_VER_PREVIEW);
      return html`
        <div class="crumb">
          <button type="button" @click=${() => { this.vanillaStep = "version"; this.loader = "vanilla"; this.loaderVer = ""; }}>← 重选版本</button>
          <span>/</span>
          <strong>${this.pick}</strong>
          <span>· ① 选择加载器</span>
        </div>
        <div class="loader-grid" role="listbox" aria-label="加载器">
          ${loaders.map(
            (l) => html`<button
              class="loader-card ${this.loader === l.id ? "on" : ""}"
              type="button"
              role="option"
              aria-selected=${this.loader === l.id}
              @click=${() => this.pickLoader(l.id)}
            >
              <span class="loader-ico ${l.id}" aria-hidden="true">${loaderIcon(l.id)}</span>
              <span>
                <div class="lname">${l.name}</div>
                <div class="ldesc">${l.desc}</div>
              </span>
            </button>`,
          )}
        </div>
        ${this.loader === "vanilla"
          ? html`<div class="loader-panel">
                <div class="muted">将安装原版 ${this.pick} · 不加模组加载器</div>
                <div class="row">
                  <input .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} placeholder="给这个版本起个名字（可选）" style="flex:1;min-width:140px" aria-label="版本名称" />
                  <button class="primary" type="button" ?disabled=${this.busy} @click=${() => this.installVanilla()}>
                    ${this.busy ? "安装中…" : "安装并添加版本"}
                  </button>
                </div>
              </div>`
          : this.loaderVersLoading
            ? html`<div class="loader-panel">
                <div class="panel-head"><span>正在拉取 ${loaderLabel(this.loader)} 版本…</span></div>
                <div class="loader-skel" aria-busy="true" aria-label="正在加载加载器版本">
                  ${[1, 2, 3, 4, 5, 6].map(() => html`<div class="skel skel-block"></div>`)}
                </div>
              </div>`
            : this.loaderVers.length
              ? html`<div class="loader-panel">
              <div class="panel-head">
                <span>② 点选加载器版本 · ${loaderLabel(this.loader)}</span>
                <span class="muted">共 ${this.loaderVers.length} 个</span>
              </div>
              <div class="ver-cards" role="listbox" aria-label="加载器版本">
                ${shown.map(
                  (v) => html`<button
                    class="ver-card lv ${this.loaderVer === v.version ? "on" : ""}"
                    type="button"
                    role="option"
                    aria-selected=${this.loaderVer === v.version}
                    @click=${() => (this.loaderVer = v.version)}
                  >
                    <span class="vid">${v.version}</span>
                    ${v.stable ? html`<span class="badge-stable">稳定</span>` : nothing}
                  </button>`,
                )}
              </div>
              ${this.loaderVers.length > LOADER_VER_PREVIEW
                ? html`<button class="back" type="button" @click=${() => (this.loaderVerExpanded = !this.loaderVerExpanded)}>
                    ${this.loaderVerExpanded ? "收起" : `显示全部 ${this.loaderVers.length} 个`}
                  </button>`
                : nothing}
              <p class="loader-cta-hint">${this.loaderVer ? `已选 ${this.loaderVer}，可以安装了` : "先点选上方一个版本，再点安装"}</p>
              <div class="row">
                <input .value=${this.newName} @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)} placeholder="给这个版本起个名字（可选）" style="flex:1;min-width:140px" aria-label="版本名称" />
                <button class="primary" type="button" ?disabled=${this.busy || !this.loaderVer} @click=${() => this.installVanilla()}>
                  ${this.busy ? "安装中…" : "安装并添加版本"}
                </button>
              </div>
            </div>`
              : html`<div class="results-slot"><easymc-empty-state
                  variant="panel"
                  title="咦？这个加载器好像还没版本…"
                  description="换个游戏版本或加载器再试试"
                >
                  <button class="ghost" type="button" @click=${() => { this.vanillaStep = "version"; this.loader = "vanilla"; this.loaderVer = ""; }}>重选游戏版本</button>
                </easymc-empty-state></div>`}
      `;
    }

    const typeLabel = (v: VersionSummary) => {
      const ty = versionType(v);
      if (ty === "snapshot") return "快照";
      if (ty === "old_beta" || ty === "old_alpha") return "远古";
      return "正式版";
    };

    return html`
      <div class="vanilla-body">
        <div class="toolbar">
          <div class="search grow">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg>
            <input placeholder="筛选版本号…" .value=${this.filter} aria-label="筛选版本号" @input=${(e: Event) => (this.filter = (e.target as HTMLInputElement).value)} />
            ${this.filter.trim()
              ? html`<button class="clear" type="button" title="清空筛选" @click=${() => { this.filter = ""; }}>清空</button>`
              : nothing}
          </div>
          <label class="muted" style="display:inline-flex;align-items:center;gap:6px">
            <input type="checkbox" .checked=${this.snapshots} @change=${async (e: Event) => { this.snapshots = (e.target as HTMLInputElement).checked; await this.loadVanilla(); }} />
            含快照
          </label>
        </div>
        ${this.vanillaLoading
          ? html`<div class="vanilla-skel" aria-busy="true" aria-label="正在加载版本">
              ${[1, 2, 3, 4, 5, 6].map(() => html`<div class="skel skel-block ver-skel"></div>`)}
            </div>`
          : !filtered.length
            ? html`<div class="results-slot">
                <easymc-empty-state
                  variant="panel"
                  title="没有符合筛选的版本"
                  description="换个关键词试试，或者打开「含快照」看看"
                >
                  <button class="ghost" type="button" @click=${() => { this.filter = ""; }}>清空筛选</button>
                </easymc-empty-state>
              </div>`
            : html`<div class="ver-grid" role="listbox" aria-label="游戏版本">
                ${filtered.slice(0, 48).map(
                  (v) => html`<button
                    class="ver-card"
                    type="button"
                    role="option"
                    @click=${() => this.pickVersion(v.id)}
                  >
                    <span>
                      <div class="vid">${v.id}</div>
                      <div class="meta">${typeLabel(v)}</div>
                    </span>
                    ${v.installed ? html`<span class="badge-inst">已安装</span>` : nothing}
                  </button>`,
                )}
              </div>`}
      </div>
    `;
  }

  private async pickVersion(id: string) {
    this.pick = id;
    this.loader = "vanilla";
    this.loaderVer = "";
    this.loaderVers = [];
    this.loaderVerExpanded = false;
    this.vanillaStep = "loader";
  }

  private async pickLoader(id: LoaderId) {
    this.loader = id;
    this.loaderVerExpanded = false;
    if (id === "vanilla") {
      this.loaderVers = [];
      this.loaderVer = "";
      this.loaderVersLoading = false;
      return;
    }
    this.loaderVersLoading = true;
    this.loaderVers = [];
    this.loaderVer = "";
    try {
      this.loaderVers = await listLoaderVersions(id, this.pick);
      this.loaderVer = this.loaderVers[0]?.version ?? "";
    } finally {
      this.loaderVersLoading = false;
    }
  }

  private async installVanilla() {
    if (this.loader !== "vanilla" && !this.loaderVer) {
      showToast("先点选一个加载器版本");
      return;
    }
    this.busy = true;
    try {
      const name = this.newName.trim() || `我的世界 ${this.pick}${this.loader === "vanilla" ? "" : ` (${loaderLabel(this.loader)})`}`;
      const inst = await createInstance(name, this.pick, this.loader, this.loader === "vanilla" ? null : this.loaderVer || null);
      if (this.loader === "vanilla") await installVersion(this.pick, true);
      else await installLoaderVersion(this.pick, this.loader, this.loaderVer || null, true);
      await setActiveInstance(inst.id);
      const st = await listInstancesState();
      this.instances = st.instances;
      this.activeId = st.active_id;
      showToast(`已装好「${inst.name}」，可以去启动页开玩`);
    } catch (e) {
      showToast(friendlyError(e, "安装没成功。请在 EasyMC 应用里重试，或换个下载源后再试"));
    } finally {
      this.busy = false;
    }
  }

  private renderMarket() {
    if (this.pickProject) return this.renderVersionPick(this.pickProject);
    const hint =
      this.tab === "mod"
        ? "为当前要玩的版本找模组"
        : this.tab === "modpack"
          ? "一键装好整套模组包"
          : this.tab === "resourcepack"
            ? "换皮肤材质包"
            : this.tab === "shader"
              ? "让画面更好看"
              : "给世界加玩法";
    const needsInst = marketNeedsInstance(this.tab as MarketTab);
    const emptyCopy = marketEmptyTitle(this.query);
    return html`
      <p class="muted" style="margin-top:0">${hint}</p>
      <div class="market-chrome">
        <div class="search-row">
          <div class="search">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/></svg>
            <input
              placeholder=${this.tab === "modpack" ? "搜索整合包…（留空看热门）" : "搜索…（留空看热门）"}
              .value=${this.query}
              aria-label="搜索"
              @keydown=${(e: KeyboardEvent) => { if (e.key === "Enter") void this.onSearch(); }}
              @input=${(e: Event) => (this.query = (e.target as HTMLInputElement).value)}
            />
            ${this.query.trim()
              ? html`<button class="clear" type="button" title="清空" @click=${() => { this.query = ""; void this.onSearch(); }}>清空</button>`
              : nothing}
          </div>
          <button class="primary" type="button" ?disabled=${this.loading} @click=${() => this.onSearch()}>
            ${this.loading ? "搜索中…" : "搜索"}
          </button>
          ${this.instances.length
            ? html`<select .value=${this.activeId ?? ""} @change=${async (e: Event) => { this.activeId = (e.target as HTMLSelectElement).value; await setActiveInstance(this.activeId); }} title="装进哪个版本" aria-label="装进哪个版本">
                ${this.instances.map((i) => html`<option value=${i.id}>${i.name}${i.version_ready ? "" : "（未装好）"}</option>`)}
              </select>`
            : nothing}
        </div>
        <div class="sources">
          <span class="lbl">来源</span>
          <button class="md-chip ${this.source === "modrinth" ? "on" : ""}" type="button" aria-pressed=${this.source === "modrinth"} @click=${() => { this.source = "modrinth"; void this.onSearch(); }}>
            <span class="dot mr"></span> Modrinth
          </button>
          <button class="md-chip ${this.source === "curseforge" ? "on" : ""}" type="button" aria-pressed=${this.source === "curseforge"} @click=${() => { this.source = "curseforge"; void this.onSearch(); }}>
            <span class="dot cf"></span> CurseForge
          </button>
        </div>
      </div>
      ${needsInst && !this.instances.length
        ? html`<div class="need-inst" role="status">
            <span class="grow">装模组 / 资源包前，需要先有一个游戏版本。</span>
            <button class="primary" type="button" @click=${() => this.setTab("vanilla")}>去装原版或加载器</button>
            <button class="ghost" type="button" @click=${() => this.setTab("modpack")}>或装整合包</button>
          </div>`
        : nothing}
      ${this.source === "curseforge" && !this.cf?.configured
        ? html`<div class="panel" style="margin-bottom:12px">
            <p class="muted">浏览 CurseForge 需要密钥（可在
              <a href="https://console.curseforge.com/" target="_blank" rel="noreferrer">CurseForge 控制台</a> 申请）。Modrinth 无需密钥。</p>
            <div class="bar">
              <input type="password" placeholder="粘贴密钥…" .value=${this.cfKeyDraft} @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)} aria-label="CurseForge 密钥" />
              <button class="primary" type="button" @click=${async () => { const r = await setCurseforgeKey(this.cfKeyDraft.trim() || null); showToast(r.message); this.cf = await curseforgeConfigured(); }}>保存密钥</button>
              <button class="ghost" type="button" @click=${() => { this.source = "modrinth"; void this.onSearch(); }}>改用 Modrinth</button>
            </div>
          </div>`
        : nothing}
      ${this.tab === "datapack"
        ? html`<div class="bar"><label class="muted">装到哪个世界的数据包文件夹（可留空=版本级 datapacks）</label>
            <input placeholder="世界名（可选）" .value=${this.worldPick} @input=${(e: Event) => (this.worldPick = (e.target as HTMLInputElement).value)} list="worlds" />
            <datalist id="worlds">${this.worlds.map((w) => html`<option value=${w}></option>`)}</datalist>
          </div>`
        : nothing}
      <div class="market-body">
        ${this.loading
          ? html`<div class="market-skel" aria-busy="true" aria-label="正在搜索">
              ${[1, 2, 3, 4, 5, 6].map(() => html`<div class="skel skel-block card-skel"></div>`)}
            </div>`
          : this.error
            ? html`<div class="results-slot"><easymc-empty-state
                variant="panel"
                title="模组列表暂时加载不了"
                description=${this.error}
              >
                <button class="primary" type="button" @click=${() => void this.onSearch()}>再试一次</button>
              </easymc-empty-state></div>`
            : this.results.length === 0
              ? html`<div class="results-slot"><easymc-empty-state
                variant="panel"
                title=${emptyCopy.title}
                description=${emptyCopy.description}
              >
                <button class="ghost" type="button" @click=${() => { this.query = ""; void this.onSearch(); }}>重新加载热门</button>
              </easymc-empty-state></div>`
              : html`
                  <div class="results-meta">
                    <span>${this.query.trim() ? `找到 ${this.results.length} 个结果` : `热门 · ${this.results.length} 个`}</span>
                    ${this.active && needsInst ? html`<span>将装进：${this.active.name}</span>` : nothing}
                  </div>
                  <div class="market-grid">
                    ${this.results.map((p) => this.renderCard(p))}
                  </div>`}
      </div>
    `;
  }

  private renderCard(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const cta = marketInstallCta(this.tab as MarketTab);
    const needsInst = marketNeedsInstance(this.tab as MarketTab);
    const blocked = needsInst && !this.active;
    const dl =
      typeof p.downloads === "number"
        ? p.downloads >= 1000
          ? `${(p.downloads / 1000).toFixed(p.downloads >= 10000 ? 0 : 1)}K`
          : String(p.downloads)
        : String(p.downloads ?? "");
    return html`<div class="card">
      <div class="card-top">
        ${p.icon_url ? html`<img src=${p.icon_url} alt="" />` : html`<div class="ph" aria-hidden="true"></div>`}
        <div class="body">
          <div class="name">${p.name}</div>
          <div class="desc">${p.description}</div>
          <div class="tags">
            <span class="tag">${p.source}</span>
            ${p.author ? html`<span class="tag">${p.author}</span>` : nothing}
          </div>
        </div>
      </div>
      <div class="card-foot">
        <span class="dl-stat">${dl ? `↓ ${dl}` : ""}</span>
        <button
          class="primary"
          type="button"
          ?disabled=${!!busy || blocked}
          title=${blocked ? "请先添加一个游戏版本" : ""}
          @click=${() => void this.openPick(p)}
        >${busy ? "安装中…" : cta}</button>
      </div>
    </div>`;
  }

  private renderVersionPick(p: ModProjectDto) {
    const busy = this.installingId === p.id;
    const needsInst = marketNeedsInstance(this.tab as MarketTab);
    const tabLabel =
      this.tab === "mod"
        ? "模组"
        : this.tab === "modpack"
          ? "整合包"
          : this.tab === "resourcepack"
            ? "资源包"
            : this.tab === "shader"
              ? "光影"
              : "数据包";
    const gameBits = (v: ModVersionDto) => {
      const gv = (v.game_versions || []).slice(0, 3).join(", ");
      const ld = (v.loaders || []).slice(0, 2).join(", ");
      const parts = [gv, ld].filter(Boolean);
      return parts.length ? parts.join(" · ") : v.name;
    };
    return html`
      <div class="crumb">
        <button type="button" @click=${() => this.closePick()}>← 返回${tabLabel}列表</button>
        <span>/</span>
        <strong>${p.name}</strong>
        <span>· 选择版本</span>
      </div>
      <div class="pick-panel">
        <div class="pick-hero">
          ${p.icon_url ? html`<img src=${p.icon_url} alt="" />` : html`<div class="ph" aria-hidden="true"></div>`}
          <div class="body">
            <h2 class="name">${p.name}</h2>
            <p class="desc">${p.description || ""}</p>
            <div class="pick-meta">
              ${p.source}${p.author ? ` · ${p.author}` : ""}
              ${this.active && needsInst ? html` · 将装进「${this.active.name}」` : nothing}
            </div>
          </div>
        </div>
        ${this.versionsLoading
          ? html`<div class="pick-versions" aria-busy="true" aria-label="正在加载版本">
              ${[1, 2, 3, 4].map(() => html`<div class="skel skel-block ver-skel"></div>`)}
            </div>`
          : this.versions.length === 0
            ? html`<p class="muted">暂时没有可装的版本。换个来源或游戏版本再试。</p>`
            : html`<div class="pick-versions" role="listbox" aria-label="可选版本">
                ${this.versions.slice(0, 40).map(
                  (v) => html`<button
                    class="pick-ver ${this.pickVersionId === v.id ? "on" : ""}"
                    type="button"
                    role="option"
                    aria-selected=${this.pickVersionId === v.id}
                    @click=${() => (this.pickVersionId = v.id)}
                  >
                    <span class="vid">${v.version_number}</span>
                    <span class="vmeta">${gameBits(v)}</span>
                  </button>`,
                )}
              </div>`}
        <div class="pick-foot">
          <span class="hint">${this.pickVersionId ? "已选版本，确认后开始安装" : "先点选一个版本"}</span>
          <button class="ghost" type="button" @click=${() => this.closePick()}>取消</button>
          <button
            class="primary"
            type="button"
            ?disabled=${!!busy || !this.pickVersionId || this.versionsLoading}
            @click=${() => void this.onInstall(p, this.pickVersionId)}
          >${busy ? "安装中…" : "确认安装"}</button>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-download-page": DownloadPage;
  }
}
