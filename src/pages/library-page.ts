import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import type { InstalledModEntryDto, InstanceDto, VersionSummary } from "../types";
import { versionId, versionType } from "../types";
import {
  listInstancesState,
  listVersions,
  createInstance,
  setActiveInstance,
  installVersion,
  installLoaderVersion,
  listLoaderVersions,
  ensureJava,
  listenInstallProgress,
  exportInstance,
  importInstance,
  listInstalledMods,
  removeMod,
  setModDisabled,
} from "../api";
import type { InstallProgressDto } from "../install-progress";
import {
  formatProgressLine,
  overallPercent,
  stageLabel,
  stagePercent,
} from "../install-progress";
import { loaderLabel } from "../types";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import { instanceIconUrl } from "../lib/instance-icon";
import "../components/empty-state";

@customElement("easymc-library-page")
export class LibraryPage extends LitElement {
  @state() private instances: InstanceDto[] = [];
  @state() private activeId: string | null = null;
  @state() private loading = true;
  @state() private busy = false;
  @state() private status = "";

  @state() private showCreate = false;
  @state() private newName = "";
  @state() private newVersion = "1.21.1";
  @state() private includeSnapshots = false;
  @state() private versions: VersionSummary[] = [];
  @state() private versionsLoading = false;
  @state() private versionFilter = "";
  @state() private alsoInstall = true;
  @state() private withAssets = true;
  @state() private newLoader: "vanilla" | "fabric" | "quilt" | "forge" | "neoforge" = "vanilla";
  @state() private loaderVersions: { version: string; stable: boolean; intermediary: string }[] = [];
  @state() private newLoaderVersion = "";
  @state() private progress: InstallProgressDto | null = null;

  /** Instance whose mods panel is open (PCL-like: mods live under the instance). */
  @state() private detailId: string | null = null;
  @state() private mods: InstalledModEntryDto[] = [];
  @state() private modsLoading = false;
  @state() private busyMod: string | null = null;
  @state() private modFilter: "all" | "enabled" | "disabled" = "all";
  @state() private removingMod: string | null = null;

  private unProg?: () => void;

  static styles = css`
    :host {
      display: flex;
      flex-direction: column;
      flex: 1 0 auto;
      min-height: 100%;
      width: 100%;
      box-sizing: border-box;
    }
    .head {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 12px;
      margin-bottom: 16px;
      flex-wrap: wrap;
    }
    h1 {
      margin: 0;
      font-size: var(--easymc-type-display);
      font-weight: var(--easymc-weight-bold);
    }
    .sub {
      margin: 4px 0 0;
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-label);
    }
    .btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
      padding: 10px 18px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      cursor: pointer;
    }
    .btn:disabled {
      opacity: 0.55;
      cursor: not-allowed;
    }
    .btn.primary {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .btn.ghost {
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface);
    }
    .btn.sm {
      padding: 6px 12px;
      font-size: var(--easymc-type-meta);
    }
    .grid {
      flex: 1 1 auto;
      min-height: 200px;
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-tile), 100%), 1fr));
      gap: var(--easymc-grid-gap);
    }
    @container easymc-page (max-width: 720px) {
      .grid {
        grid-template-columns: repeat(
          auto-fill,
          minmax(min(var(--easymc-grid-min-tile-narrow), 100%), 1fr)
        );
      }
    }
    .card {
      padding: var(--easymc-space-3) var(--easymc-space-3);
      border-radius: var(--easymc-card-radius);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: none;
      display: flex;
      flex-direction: column;
      gap: var(--easymc-space-2);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      min-width: 0;
      height: 100%;
      box-sizing: border-box;
    }
    .card.active {
      border: 2px solid var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
    }
    .card-thumb {
      width: 48px;
      height: 48px;
      border-radius: var(--md-sys-shape-corner-medium);
      object-fit: cover;
      flex-shrink: 0;
      background: var(--md-sys-color-surface-container);
      display: block;
    }
    .card h3 {
      margin: 0;
      font-size: var(--easymc-type-body);
    }
    .meta {
      font-size: var(--easymc-type-label);
      color: var(--md-sys-color-on-surface-variant);
    }
    .badge {
      display: inline-block;
      padding: 2px 10px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
      background: var(--md-sys-color-surface-container-highest);
    }
    .badge.ok {
      background: var(--md-sys-color-success-container);
      color: var(--md-sys-color-on-success-container);
    }
    .badge.warn {
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
    }
    .badge.def {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .row {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
      margin-top: auto;
      padding-top: 8px;
    }
    .status {
      margin: 0 0 14px;
      font-size: var(--easymc-type-label);
      color: var(--md-sys-color-on-surface-variant);
      min-height: 1.2em;
    }
    .panel {
      margin-bottom: 20px;
      padding: 20px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    .panel h2 {
      margin: 0;
      font-size: var(--easymc-type-title);
    }
    label {
      display: flex;
      flex-direction: column;
      gap: 6px;
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface-variant);
    }
    input[type="text"],
    select {
      padding: 10px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
    }
    .toolbar {
      display: flex;
      gap: 10px;
      flex-wrap: wrap;
      align-items: center;
    }
    .check {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface-variant);
      cursor: pointer;
      flex-direction: row;
    }
    .ver-list {
      max-height: 220px;
      overflow: auto;
      border: 1px solid var(--md-sys-color-outline-variant);
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface);
    }
    .ver-item {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      width: 100%;
      padding: 10px 14px;
      border: none;
      border-bottom: 1px solid var(--md-sys-color-outline-variant);
      background: transparent;
      text-align: left;
      cursor: pointer;
      font: inherit;
      color: var(--md-sys-color-on-surface);
    }
    .ver-item:last-child {
      border-bottom: none;
    }
    .ver-item:hover {
      background: var(--md-sys-color-surface-container);
    }
    .ver-item.selected {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
    }
    .ver-item .right {
      display: flex;
      gap: 6px;
      align-items: center;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .empty-wrap {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      align-self: stretch;
      min-height: 240px;
      width: 100%;
      grid-column: 1 / -1;
    }
    .ver-empty {
      padding: 20px 12px;
      text-align: center;
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-label);
      line-height: 1.5;
    }
    .progress-card {
      margin: 0 0 16px;
      padding: 14px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
    }
    .progress-card.error {
      border: 1px solid color-mix(in oklch, var(--md-sys-color-error) 40%, transparent);
      background: var(--md-sys-color-error-container);
    }
    .progress-head {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      align-items: baseline;
      margin-bottom: 8px;
      font-size: var(--easymc-type-label);
      font-weight: var(--easymc-weight-medium);
    }
    .progress-head .muted {
      font-weight: var(--easymc-weight-medium);
      color: var(--md-sys-color-on-surface-variant);
      font-size: var(--easymc-type-meta);
    }
    .bar {
      height: 8px;
      border-radius: 999px;
      background: var(--md-sys-color-surface-container-highest);
      overflow: hidden;
    }
    .bar > i {
      display: block;
      height: 100%;
      background: var(--md-sys-color-primary);
      border-radius: 999px;
      transition: width 160ms ease;
    }
    .bar.error > i {
      background: var(--md-sys-color-error);
    }
    .progress-detail {
      margin-top: 8px;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      word-break: break-all;
    }

    .loader-row {
      display: flex;
      flex-direction: column;
      gap: 8px;
      margin-bottom: 12px;
    }
    .field-label {
      font-size: var(--easymc-type-label);
      color: var(--md-sys-color-on-surface-variant);
    }
    .chip-row {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }
    .chip {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
      border-radius: 999px;
      padding: 6px 14px;
      font: inherit;
      cursor: pointer;
    }
    .chip.on {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border-color: var(--md-sys-color-primary);
    }
    .field {
      display: flex;
      flex-direction: column;
      gap: 6px;
      margin-bottom: 12px;
      font-size: var(--easymc-type-label);
    }
    .field select {
      font: inherit;
      padding: 8px 10px;
      border-radius: var(--md-sys-shape-corner-small); /* CMP-5: form field */
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface);
    }

    .detail {
      margin-top: 20px;
      padding: 20px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
    }
    .detail-head {
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 14px;
    }
    .detail-head h2 {
      margin: 0;
      font-size: var(--easymc-type-title);
      letter-spacing: -0.02em;
    }
    .detail-head .meta {
      margin-top: 4px;
    }
    .mod-toolbar {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      align-items: center;
      margin-bottom: 12px;
    }
    .mod-toolbar .spacer {
      flex: 1;
    }
    .mod-rows {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(min(var(--easymc-grid-min-dense), 100%), 1fr));
      gap: var(--easymc-grid-gap);
    }
    @container easymc-page (max-width: 720px) {
      .mod-rows {
        grid-template-columns: repeat(
          auto-fill,
          minmax(min(var(--easymc-grid-min-dense-narrow), 100%), 1fr)
        );
      }
    }
    .mod-row {
      display: flex;
      flex-direction: column;
      gap: 10px;
      padding: 12px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      box-shadow: var(--md-sys-elevation-1);
      font-size: var(--easymc-type-label);
      min-height: 0;
    }
    .mod-row.leaving {
      animation: row-out var(--easymc-dur-fast) var(--easymc-ease-in) both;
    }
    .mod-row.disabled { opacity: 0.62; }
    .mod-row-top {
      display: flex;
      gap: 10px;
      align-items: flex-start;
    }
    .mod-row img, .mod-row .ph {
      width: 44px;
      height: 44px;
      border-radius: var(--md-sys-shape-corner-large);
      object-fit: cover;
      background: var(--md-sys-color-primary-container);
      flex-shrink: 0;
    }
    .mod-row .name {
      flex: 1;
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-weight: var(--easymc-weight-medium);
      font-size: var(--easymc-type-body);
    }
    .mod-row .meta {
      margin-top: 2px;
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
    }
    .mod-row .actions {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      margin-top: auto;
    }
    .mod-row .badge.off {
      font-size: var(--easymc-type-meta);
      padding: 2px 8px;
      border-radius: 999px;
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
      flex-shrink: 0;
    }
    .mod-row button.danger {
      padding: 6px 12px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-error);
      color: var(--md-sys-color-on-error);
      font: inherit;
      font-size: var(--easymc-type-label);
      cursor: pointer;
      flex-shrink: 0;
    }
    .mod-row button.danger:disabled,
    .mod-row button.ghost:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
    .mod-empty-slot {
      flex: 1 1 auto;
      display: flex;
      flex-direction: column;
      min-height: 200px;
      background: var(--md-sys-color-surface-container-lowest);
      border-radius: var(--md-sys-shape-corner-large);
    }
    .card {
      cursor: pointer;
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        border-color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .card:hover {
      background: var(--md-sys-color-surface-container-high);
    }
    .card.selected {
      border-color: var(--md-sys-color-primary);
    }
    .card:active {
      transform: scale(0.99);
    }
    .btn {
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        opacity var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .btn:active:not(:disabled) {
      transform: scale(0.98);
    }
    .chip {
      transition:
        background var(--easymc-dur-press) var(--easymc-ease-out),
        color var(--easymc-dur-press) var(--easymc-ease-out),
        transform var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .chip:active {
      transform: scale(0.98);
    }
    .chip.on {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border-color: transparent;
    }
    .badge.ok {
      background: var(--md-sys-color-success-container);
      color: var(--md-sys-color-on-success-container);
    }
    .badge.warn {
      background: var(--md-sys-color-warning-container);
      color: var(--md-sys-color-on-warning-container);
    }
    @keyframes row-out {
      from { opacity: 1; transform: none; }
      to { opacity: 0; transform: translateX(12px); }
    }
    @media (prefers-reduced-motion: reduce) {
      .mod-row.leaving { animation: none; opacity: 0; }
      .card:active, .btn:active:not(:disabled), .chip:active { transform: none; }
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.load();
    void listenInstallProgress((p) => {
      this.progress = p;
      this.status = formatProgressLine(p);
    }).then((u) => (this.unProg = u));
  }

  disconnectedCallback() {
    this.unProg?.();
    super.disconnectedCallback();
  }

  private async load() {
    this.loading = true;
    const state = await listInstancesState();
    this.instances = state.instances;
    this.activeId = state.active_id;
    this.loading = false;
    if (!this.detailId && state.active_id) {
      await this.openDetail(state.active_id);
    } else if (this.detailId) {
      if (!state.instances.some((i) => i.id === this.detailId)) {
        this.detailId = state.active_id;
      }
      if (this.detailId) await this.refreshMods();
    }
  }

  private async openCreate() {
    this.showCreate = true;
    this.newName = "";
    this.newVersion = "1.21.1";
    this.newLoader = "vanilla";
    this.newLoaderVersion = "";
    this.loaderVersions = [];
    this.versionFilter = "";
    await this.refreshVersions();
  }

  private async refreshVersions() {
    this.versionsLoading = true;
    try {
      this.versions = await listVersions(this.includeSnapshots);
      if (!this.versions.find((v) => v.id === this.newVersion) && this.versions[0]) {
        this.newVersion = this.versions[0].id;
      }
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.versionsLoading = false;
    }
  }

  private filteredVersions(): VersionSummary[] {
    const q = this.versionFilter.trim().toLowerCase();
    if (!q) return this.versions;
    return this.versions.filter(
      (v) => v.id.toLowerCase().includes(q) || versionType(v).toLowerCase().includes(q),
    );
  }


  private async refreshLoaderVersions() {
    if (this.newLoader === "vanilla") {
      this.loaderVersions = [];
      this.newLoaderVersion = "";
      return;
    }
    try {
      const list = await listLoaderVersions(this.newLoader, this.newVersion);
      this.loaderVersions = list;
      const stable = list.find((l) => l.stable);
      this.newLoaderVersion = (stable ?? list[0])?.version ?? "";
    } catch (e) {
      this.loaderVersions = [];
      this.newLoaderVersion = "";
      const msg = friendlyError(e);
      showToast(msg);
    }
  }

  private async onLoaderChange(loader: "vanilla" | "fabric" | "quilt" | "forge" | "neoforge") {
    this.newLoader = loader;
    await this.refreshLoaderVersions();
  }

  private async onCreate() {
    const name = this.newName.trim() || `实例 ${this.newVersion}`;
    const version = this.newVersion;
    if (!version) {
      showToast("请选择版本");
      return;
    }
    this.busy = true;
    this.status = `正在创建实例「${name}」…`;
    try {
      const inst = await createInstance(name, version, this.newLoader, this.newLoaderVersion || null);
      showToast(`已创建 ${inst.name}`);
      if (this.alsoInstall && !inst.version_ready) {
        this.progress = null;
        const loaderTip =
          this.newLoader === "vanilla" ? "" : ` · ${loaderLabel(this.newLoader)}`;
        this.status = `正在安装 ${version}${loaderTip}…`;
        await ensureJava(21);
        const msg =
          this.newLoader === "vanilla"
            ? await installVersion(version, this.withAssets)
            : await installLoaderVersion(
                version,
                this.newLoader,
                this.newLoaderVersion || null,
                this.withAssets,
              );
        showToast(msg);
        this.status = msg;
      } else if (this.alsoInstall && inst.version_ready) {
        this.status = `已创建 · ${loaderLabel(this.newLoader)} ${versionId(inst.version)} 已在本地，跳过下载`;
      } else {
        this.status = `已创建 · ${versionId(inst.version)} · ${loaderLabel(this.newLoader)}`;
      }
      this.showCreate = false;
      await this.load();
    } catch (e) {
      const msg = friendlyError(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }

  private async onSetActive(id: string) {
    try {
      const state = await setActiveInstance(id);
      this.instances = state.instances;
      this.activeId = state.active_id;
      showToast("已设为默认实例");
    } catch (e) {
      showToast(friendlyError(e));
    }
  }

  private async onInstall(inst: InstanceDto, withAssets = this.withAssets) {
    const ver = versionId(inst.version);
    this.busy = true;
    this.progress = null;
    const mode = withAssets ? "含完整资源" : "跳过资源";
    this.status = `正在安装 ${ver}（${mode}）…`;
    try {
      await ensureJava(21);
      const msg = await installVersion(ver, withAssets);
      showToast(msg);
      this.status = msg;
      await this.load();
    } catch (e) {
      const msg = friendlyError(e);
      this.status = msg;
      showToast(msg);
    } finally {
      this.busy = false;
    }
  }



  private get detail(): InstanceDto | undefined {
    return this.instances.find((i) => i.id === this.detailId);
  }

  private async openDetail(id: string) {
    this.detailId = id;
    this.modFilter = "all";
    await this.refreshMods();
  }

  private async refreshMods() {
    const id = this.detailId;
    if (!id) {
      this.mods = [];
      return;
    }
    this.modsLoading = true;
    try {
      this.mods = await listInstalledMods(id);
    } catch (e) {
      showToast(friendlyError(e));
      this.mods = [];
    } finally {
      this.modsLoading = false;
    }
  }

  private filteredMods(): InstalledModEntryDto[] {
    if (this.modFilter === "enabled") {
      return this.mods.filter((m) => !m.disabled);
    }
    if (this.modFilter === "disabled") {
      return this.mods.filter((m) => m.disabled);
    }
    return this.mods;
  }

  private goStore() {
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: "download", bubbles: true, composed: true }),
    );
  }

  private async onUninstall(filename: string) {
    if (!this.detailId) return;
    if (!window.confirm(`卸载模组？\n${filename}`)) return;
    this.busyMod = filename;
    this.removingMod = filename;
    try {
      const removed = await removeMod(filename, this.detailId);
      showToast(`已卸载 ${removed}`);
      // brief leave animation then refresh
      await new Promise((r) => setTimeout(r, 160));
      await this.refreshMods();
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.busyMod = null;
      this.removingMod = null;
    }
  }

  private async onToggleDisabled(filename: string) {
    if (!this.detailId) return;
    const disabled = !filename.endsWith(".jar.disabled");
    this.busyMod = filename;
    try {
      const next = await setModDisabled(filename, disabled, this.detailId);
      showToast(disabled ? `已禁用 ${next}` : `已启用 ${next}`);
      await this.refreshMods();
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.busyMod = null;
    }
  }

  private async onExport(inst: InstanceDto, pack: boolean) {
    this.busy = true;
    this.status = pack ? "正在导出 .mrpack 整合包…" : "正在导出分享配置…";
    try {
      const res = await exportInstance(inst.id, pack);
      this.status = res.message_zh;
      showToast(res.path);
      try {
        await navigator.clipboard.writeText(res.path);
        showToast("路径已复制到剪贴板");
      } catch {
        /* clipboard may be unavailable */
      }
    } catch (e) {
      this.status = friendlyError(e);
      showToast(friendlyError(e));
    } finally {
      this.busy = false;
    }
  }

  private async onImport() {
    const path = window.prompt(
      "导入路径（EasyMC .json/.zip，或 Modrinth .mrpack）",
      "",
    );
    if (!path?.trim()) return;
    const applyMem = window.confirm("是否将分享包中的内存建议写入本地 settings？");
    this.busy = true;
    this.status = "正在导入…";
    try {
      const res = await importInstance(path.trim(), { applyMemory: applyMem });
      this.status = res.message_zh;
      showToast(res.message_zh.slice(0, 80));
      const st = await listInstancesState();
      this.instances = st.instances;
      this.activeId = st.active_id;
    } catch (e) {
      this.status = friendlyError(e);
      showToast(friendlyError(e));
    } finally {
      this.busy = false;
    }
  }

  render() {
    if (this.loading) return html`<p class="sub" aria-busy="true" style="margin:24px 0;color:var(--md-sys-color-on-surface-variant)">正在准备你的库…</p>`;
    return html`
      <div class="head">
        <div>
          <h1>我的库</h1>
          <p class="sub">管理你的版本与模组；想找新内容去「下载」页</p>
        </div>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button class="btn ghost" type="button" ?disabled=${this.busy} @click=${() => this.onImport()}>
            导入别人的分享
          </button>
          <button class="btn primary" type="button" ?disabled=${this.busy} @click=${() => this.openCreate()}>
            ＋ 新建版本
          </button>
        </div>
      </div>

      <p class="status">${this.status}</p>
      ${this.renderProgress()}

      ${this.showCreate ? this.renderCreate() : null}

      <div class="grid">
        ${this.instances.map((inst) => this.renderCard(inst))}
        ${this.instances.length === 0
          ? html`<div class="empty-wrap">
              <easymc-empty-state
                variant="panel"
                title="咦？似乎什么都没有…"
                description="还没有版本呢。点「新建版本」，或去下载中心逛逛？"
              >
                <button class="btn primary" type="button" ?disabled=${this.busy} @click=${() => this.openCreate()}>
                  新建版本
                </button>
                <button class="btn ghost" type="button" @click=${() => this.goStore()}>
                  去下载中心
                </button>
              </easymc-empty-state>
            </div>`
          : null}
      </div>

      ${this.detail ? this.renderMods(this.detail) : null}
    `;
  }

  private renderMods(inst: InstanceDto) {
    const list = this.filteredMods();
    const ver = versionId(inst.version);
    return html`
      <section class="detail" aria-label="实例模组">
        <div class="detail-head">
          <div>
            <h2>模组 · ${inst.name}</h2>
            <div class="meta">
              ${ver} · ${loaderLabel(inst.loader)}${inst.loader_version ? ` ${inst.loader_version}` : ""}
              · <code>${inst.id}</code>
            </div>
          </div>
          <button class="btn ghost sm" type="button" @click=${() => this.goStore()}>
            去下载中心
          </button>
        </div>

        <div class="mod-toolbar">
          <button
            class="chip ${this.modFilter === "all" ? "on" : ""}"
            type="button"
            @click=${() => (this.modFilter = "all")}
          >
            全部
          </button>
          <button
            class="chip ${this.modFilter === "enabled" ? "on" : ""}"
            type="button"
            @click=${() => (this.modFilter = "enabled")}
          >
            已启用
          </button>
          <button
            class="chip ${this.modFilter === "disabled" ? "on" : ""}"
            type="button"
            @click=${() => (this.modFilter = "disabled")}
          >
            已禁用
          </button>
          <span class="spacer"></span>
          <button
            class="btn ghost sm"
            type="button"
            ?disabled=${this.modsLoading}
            @click=${() => this.refreshMods()}
          >
            ${this.modsLoading ? "刷新中…" : "刷新"}
          </button>
        </div>

        ${this.modsLoading && !this.mods.length
          ? html`<p class="meta">加载模组列表…</p>`
          : list.length
            ? html`<div class="mod-rows">
                ${list.map((m) => {
                  const busy = this.busyMod === m.filename;
                  const disabled = m.disabled;
                  const leaving = this.removingMod === m.filename;
                  const label = m.filename.replace(/\.jar(\.disabled)?$/i, "");
                  const kb = Math.max(1, Math.round(m.size / 1024));
                  return html`<div class="mod-row ${leaving ? "leaving" : ""} ${disabled ? "disabled" : ""}">
                    <div class="mod-row-top">
                      ${m.icon_url
                        ? html`<img src=${m.icon_url} alt="" loading="lazy" />`
                        : html`<div class="ph" aria-hidden="true"></div>`}
                      <div style="min-width:0;flex:1">
                        <div class="name" title=${m.filename}>${label}</div>
                        <div class="meta">${kb} KB${m.source ? ` · ${m.source}` : ""}</div>
                      </div>
                      ${disabled ? html`<span class="badge off">已禁用</span>` : null}
                    </div>
                    <div class="actions">
                      <button
                        class="btn ghost sm"
                        type="button"
                        ?disabled=${busy}
                        @click=${() => this.onToggleDisabled(m.filename)}
                      >
                        ${disabled ? "启用" : "禁用"}
                      </button>
                      <button
                        class="danger"
                        type="button"
                        ?disabled=${busy}
                        @click=${() => this.onUninstall(m.filename)}
                      >
                        ${busy && leaving ? "…" : "卸载"}
                      </button>
                    </div>
                  </div>`;
                })}
              </div>`
            : html`<div class="mod-empty-slot">
                <easymc-empty-state
                  variant="panel"
                  title=${this.mods.length
                    ? "没有符合筛选的模组"
                    : "咦？还没有模组呢…"}
                  description=${this.mods.length
                    ? "换个筛选看看，或者清空后再试"
                    : "去下载中心挑几个装上吧"}
                >
                  ${this.mods.length
                    ? html`<button class="btn ghost" type="button" @click=${() => (this.modFilter = "all")}>清空筛选</button>`
                    : html`<button class="btn primary" type="button" @click=${() => this.goStore()}>去下载中心</button>`}
                </easymc-empty-state>
              </div>`}
      </section>
    `;
  }

  private renderProgress() {
    const p = this.progress;
    if (!p) return null;
    if (!this.busy && p.stage !== "error" && p.stage !== "done") return null;
    const overall = overallPercent(p);
    const local = stagePercent(p);
    const width = overall ?? local ?? 0;
    const isErr = p.stage === "error";
    const counts =
      (p.skipped ?? 0) > 0 || (p.downloaded ?? 0) > 0
        ? `新下 ${p.downloaded ?? 0} · 跳过 ${p.skipped ?? 0}`
        : "";
    return html`
      <div class="progress-card ${isErr ? "error" : ""}">
        <div class="progress-head">
          <span
            >${p.phase != null && p.phase_total != null
              ? html`${p.phase}/${p.phase_total} · `
              : null}${stageLabel(p)}</span
          >
          <span class="muted"
            >${overall != null ? `${overall.toFixed(0)}% 总进度` : local != null ? `${local.toFixed(0)}%` : ""}
            ${counts ? ` · ${counts}` : ""}</span
          >
        </div>
        <div class="bar ${isErr ? "error" : ""}"><i style="width:${width}%"></i></div>
        <div class="progress-detail">${p.detail}</div>
      </div>
    `;
  }

  private renderCreate() {
    const list = this.filteredVersions();
    return html`
      <section class="panel">
        <h2>新建实例</h2>
        <label>
          名称
          <input
            type="text"
            .value=${this.newName}
            placeholder="例如：生存 1.21"
            @input=${(e: Event) => (this.newName = (e.target as HTMLInputElement).value)}
          />
        </label>
        <div class="toolbar">
          <label style="flex:1;min-width:160px">
            筛选版本
            <input
              type="text"
              .value=${this.versionFilter}
              placeholder="搜索 1.20 / release…"
              @input=${(e: Event) => (this.versionFilter = (e.target as HTMLInputElement).value)}
            />
          </label>
          <label class="check">
            <input
              type="checkbox"
              .checked=${this.includeSnapshots}
              @change=${async (e: Event) => {
                this.includeSnapshots = (e.target as HTMLInputElement).checked;
                await this.refreshVersions();
              }}
            />
            显示快照
          </label>
          
          <div class="loader-row">
            <span class="field-label">加载器</span>
            <div class="chip-row">
              ${(["vanilla", "fabric", "quilt", "forge", "neoforge"] as const).map(
                (l) => html`
                  <button
                    type="button"
                    class="chip ${this.newLoader === l ? "on" : ""}"
                    ?disabled=${this.busy}
                    @click=${() => this.onLoaderChange(l)}
                  >
                    ${loaderLabel(l)}
                  </button>
                `,
              )}
            </div>
          </div>
          ${this.newLoader !== "vanilla"
            ? html`
                <label class="field">
                  <span>Loader 版本</span>
                  <select
                    .value=${this.newLoaderVersion}
                    ?disabled=${this.busy || this.loaderVersions.length === 0}
                    @change=${(e: Event) =>
                      (this.newLoaderVersion = (e.target as HTMLSelectElement).value)}
                  >
                    ${this.loaderVersions.length === 0
                      ? html`<option value="">还没有可选版本（稍等或换个加载器）</option>`
                      : this.loaderVersions.map(
                          (l) => html`
                            <option value=${l.version}>
                              ${l.version}${l.stable ? "（稳定）" : ""}
                            </option>
                          `,
                        )}
                  </select>
                </label>
              `
            : null}
<label class="check">
            <input
              type="checkbox"
              .checked=${this.alsoInstall}
              @change=${(e: Event) => (this.alsoInstall = (e.target as HTMLInputElement).checked)}
            />
            创建后立即安装
          </label>
          <label class="check">
            <input
              type="checkbox"
              .checked=${this.withAssets}
              @change=${(e: Event) => (this.withAssets = (e.target as HTMLInputElement).checked)}
            />
            下载游戏资源
          </label>
        </div>
        <div>
          <div class="meta" style="margin-bottom:6px">
            已选版本：<strong>${this.newVersion}</strong>
            ${this.versionsLoading ? " · 正在准备…" : ` · ${list.length} 项`}
          </div>
          <div class="ver-list">
            ${list.length === 0
              ? html`<div class="ver-empty">${this.versionsLoading ? "正在加载版本…" : "咦？没有符合筛选的版本… 换个关键词试试"}</div>`
              : list.map(
                  (v) => html`
                    <button
                      type="button"
                      class="ver-item ${v.id === this.newVersion ? "selected" : ""}"
                      @click=${() => { this.newVersion = v.id; void this.refreshLoaderVersions(); }}
                    >
                      <span>${v.id}</span>
                      <span class="right">
                        <span>${versionType(v)}</span>
                        ${v.installed ? html`<span class="badge ok">已安装</span>` : null}
                      </span>
                    </button>
                  `,
                )}
          </div>
        </div>
        <div class="row">
          <button class="btn primary" type="button" ?disabled=${this.busy} @click=${() => this.onCreate()}>
            创建
          </button>
          <button class="btn ghost" type="button" ?disabled=${this.busy} @click=${() => (this.showCreate = false)}>
            取消
          </button>
        </div>
      </section>
    `;
  }

  private renderCard(inst: InstanceDto) {
    const ver = versionId(inst.version);
    const isActive = inst.id === this.activeId;
    const selected = inst.id === this.detailId;
    return html`
      <article
        class="card ${isActive ? "active" : ""} ${selected ? "selected" : ""}"
        @click=${() => this.openDetail(inst.id)}
      >
        <div style="display:flex;gap:12px;align-items:flex-start">
          <img class="card-thumb" src=${instanceIconUrl(inst)} alt="" width="48" height="48" loading="lazy" />
          <div style="min-width:0;flex:1">
        <div>
          ${isActive ? html`<span class="badge def">默认</span>` : null}
          ${inst.version_ready
            ? html`<span class="badge ok">已安装</span>`
            : html`<span class="badge warn">未安装</span>`}
          ${selected ? html`<span class="badge def">模组</span>` : null}
        </div>
        <h3>${inst.name}</h3>
        <div class="meta">${ver} · ${loaderLabel(inst.loader)}${inst.loader_version ? ` ${inst.loader_version}` : ""}</div>
        <div class="meta">${inst.description ?? ""}</div>
          </div>
        </div>
        <div class="row" @click=${(e: Event) => e.stopPropagation()}>
          ${!isActive
            ? html`<button class="btn ghost sm" type="button" @click=${() => this.onSetActive(inst.id)}>
                设为默认
              </button>`
            : null}
          ${!inst.version_ready
            ? html`
                <button
                  class="btn primary sm"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.onInstall(inst, true)}
                >
                  安装 ${ver}
                </button>
                <button
                  class="btn ghost sm"
                  type="button"
                  ?disabled=${this.busy}
                  @click=${() => this.onInstall(inst, false)}
                >
                  高级：精简安装
                </button>
              `
            : html`<span class="badge ok">本地已有版本文件</span>`}
          <button
            class="btn ghost sm"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onExport(inst, false)}
          >
            分享给好友（仅配置）
          </button>
          <button
            class="btn ghost sm"
            type="button"
            ?disabled=${this.busy}
            @click=${() => this.onExport(inst, true)}
          >
            导出 .mrpack 整合包
          </button>
        </div>
      </article>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-library-page": LibraryPage;
  }
}
