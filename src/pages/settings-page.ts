import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  agentLlmConfigClear,
  agentLlmConfigGet,
  agentLlmConfigSave,
  agentLlmStatus,
  curseforgeConfigured,
  detectJava,
  detectMinecraftDirs,
  ensureJavaInstall,
  getSettings,
  listenLogs,
  msaStatus,
  setCurseforgeKey,
  updateSettings,
  containerOneClickSetup,
  containerPreheat,
  containerPreheatStatus,
  containerProbe,
  containerRepair,
  containerWriteRegistries,
  probeDownloadMirrors,
  probeRegistryMirrors,
  type JavaInstallDto,
  type MinecraftDirDto,
} from "../api";
import type {
  AppMode,
  ContainerPreheatStatusDto,
  ContainerRuntimeProbeDto,
  CurseForgeStatusDto,
  DownloadMirrorProbeDto,
  LlmConfigViewDto,
  LlmStatusDto,
  MsaStatusDto,
  RegistryMirrorProbeDto,
  SettingsDto,
} from "../types";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import { syncIndicator } from "../lib/indicator";

const MEM_MIN = 512;
const MEM_MAX = 16384;
const MEM_STEP = 256;

@customElement("easymc-settings-page")
export class SettingsPage extends LitElement {
  @state() private settings: SettingsDto | null = null;
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private msa: MsaStatusDto | null = null;
  @state() private mem = 2048;
  @state() private cfKeyDraft = "";
  @state() private msaDraft = "";
  @state() private dataDir = "";
  @state() private llm: LlmStatusDto | null = null;
  @state() private llmCfg: LlmConfigViewDto | null = null;
  @state() private llmBase = "";
  @state() private llmKey = "";
  @state() private llmModel = "";
  @state() private llmBusy = false;
  @state() private section: "launch" | "multiplayer" | "personal" | "other" = "launch";
  @state() private javaList: JavaInstallDto[] = [];
  @state() private mcDirs: MinecraftDirDto[] = [];
  @state() private javaBusy = false;
  @state() private preheatStatus: ContainerPreheatStatusDto | null = null;
  @state() private containerRuntimeProbe: ContainerRuntimeProbeDto | null = null;
  @state() private containerBusy = false;
  /** Which isolation CTA is running — drives per-button progress labels. */
  @state() private containerBusyAction: "probe" | "setup" | "preheat" | "registries" | "repair" | null = null;
  @state() private downloadProbes: DownloadMirrorProbeDto[] = [];
  @state() private downloadProbeBusy = false;
  @state() private registryProbes: RegistryMirrorProbeDto[] = [];
  @state() private registryProbeBusy = false;
  @state() private javaStatus = "";
  @state() private javaLog = "";
  private unlistenJavaLog: (() => void) | null = null;

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    .probe-spin {
      display: inline-block; width: 12px; height: 12px; border-radius: 50%;
      border: 2px solid color-mix(in oklch, currentColor 35%, transparent);
      border-top-color: currentColor;
      animation: easymc-probe-spin 0.7s linear infinite;
      vertical-align: -1px; margin-right: 6px;
    }
    @keyframes easymc-probe-spin { to { transform: rotate(360deg); } }
      :host {
        display: block;
        /* Column width comes from the shell (.page > * in app-shell.ts). */
      }
      h1 {
        font-size: var(--easymc-type-title);
        font-weight: var(--easymc-weight-bold);
        margin: 0 0 2px;
      }
      .sub {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-meta);
        margin: 0 0 var(--easymc-space-4);
      }
      /* .tabs → controlStyles .md-tabs (MC-2, shared vocabulary + sliding indicator) */
      .card {
        padding: var(--easymc-card-pad-y) var(--easymc-card-pad-x);
        margin-bottom: var(--easymc-grid-gap);
        border-radius: var(--easymc-card-radius);
        background: var(--md-sys-color-surface-container-lowest);
        box-shadow: var(--md-sys-elevation-1);
      }
      .card h2 {
        margin: 0 0 8px;
        font-size: var(--easymc-type-title);
      }
      .card p {
        margin: 0 0 10px;
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-body);
      }
      .row {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        align-items: center;
      }
      input:not([type="range"]),
      select {
        border: 1px solid var(--md-sys-color-outline-variant);
        background: var(--md-sys-color-surface-container-lowest);
        color: var(--md-sys-color-on-surface);
        border-radius: var(--md-sys-shape-corner-full);
        padding: 8px 14px;
        font-size: var(--easymc-type-body);
      }
      details.card summary {
        cursor: pointer;
        font-weight: var(--easymc-weight-medium);
      }
      .muted {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-label);
      }
      .mem-hints {
        display: flex;
        justify-content: space-between;
        margin-top: 4px;
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
      }
      .java-ok {
        color: var(--md-sys-color-primary);
        font-weight: var(--easymc-weight-medium);
        font-size: var(--easymc-type-label);
      }
      .java-warn {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-label);
      }
      .java-log {
        margin-top: 8px;
        padding: 10px 12px;
        border-radius: var(--md-sys-shape-corner-medium);
        background: var(--md-sys-color-surface-container-lowest);
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
        white-space: pre-wrap;
        word-break: break-all;
        max-height: 140px;
        overflow: auto;
      }
      code.path {
        font-size: var(--easymc-type-meta);
        word-break: break-all;
      }
      .mc-dirs {
        list-style: none;
        margin: 8px 0 0;
        padding: 0;
        display: grid;
        gap: 8px;
      }
      .mc-dirs li {
        display: flex;
        flex-direction: column;
        gap: 4px;
        padding: 10px 12px;
        border-radius: var(--md-sys-shape-corner-medium);
        background: var(--md-sys-color-surface-container-lowest);
      }
      .mc-tag {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-meta);
      }
      .java-list {
        list-style: none;
        margin: 12px 0 0;
        padding: 0;
        display: grid;
        gap: 8px;
      }
      .java-list li {
        margin: 0;
        padding: 0;
      }
      button.java-pick {
        width: 100%;
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        gap: 4px;
        text-align: left;
        padding: 10px 12px;
        border-radius: var(--md-sys-shape-corner-medium);
        border: 1px solid var(--md-sys-color-outline-variant);
        background: var(--md-sys-color-surface-container-lowest);
        color: inherit;
        cursor: pointer;
        font: inherit;
      }
      button.java-pick:hover {
        border-color: color-mix(in oklch, var(--md-sys-color-primary) 40%, var(--md-sys-color-outline-variant));
      }
      button.java-pick:focus-visible {
        outline: 2px solid var(--md-sys-color-primary);
        outline-offset: 2px;
      }
      button.java-pick.on {
        border-color: var(--md-sys-color-primary);
        background: color-mix(in oklch, var(--md-sys-color-primary) 12%, var(--md-sys-color-surface-container-lowest));
      }
      button.java-pick .java-pick-title {
        font-weight: var(--easymc-weight-medium);
        font-size: var(--easymc-type-label);
      }
      button.java-pick .java-pick-meta {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-meta);
      }
      button.java-pick .java-pick-path {
        font-size: var(--easymc-type-meta);
        word-break: break-all;
        opacity: 0.9;
      }
      button.java-pick .java-pick-badge {
        display: inline-block;
        margin-left: 6px;
        padding: 1px 6px;
        border-radius: 999px;
        font-size: var(--easymc-type-meta);
        background: color-mix(in oklch, var(--md-sys-color-primary) 18%, transparent);
        color: var(--md-sys-color-primary);
        font-weight: var(--easymc-weight-medium);
      }
    `,
  ];

  connectedCallback() {
    super.connectedCallback();
    const sec = sessionStorage.getItem("easymc-settings-section") as
      | "launch"
      | "multiplayer"
      | "personal"
      | "other"
      | null;
    if (sec && ["launch", "multiplayer", "personal", "other"].includes(sec)) {
      this.section = sec;
      sessionStorage.removeItem("easymc-settings-section");
    }
    void this.refresh();
  }

  disconnectedCallback() {
    this.unlistenJavaLog?.();
    this.unlistenJavaLog = null;
    super.disconnectedCallback();
  }

  protected updated() {
    syncIndicator(this.renderRoot);
  }

  private async refresh() {
    const [s, cf, msa, dir, llm, llmCfg, javas, mcDirs] = await Promise.all([
      getSettings(),
      curseforgeConfigured(),
      msaStatus(),
      agentDataDir(),
      agentLlmStatus(),
      agentLlmConfigGet(),
      detectJava(),
      detectMinecraftDirs(),
    ]);
    this.settings = s;
    this.mem = Math.min(MEM_MAX, Math.max(MEM_MIN, s.memory_mb ?? 2048));
    this.cf = cf;
    this.msa = msa;
    this.dataDir = dir.path;
    this.llm = llm;
    this.llmCfg = llmCfg;
    this.llmBase = llmCfg.base_url || "";
    this.llmModel = llmCfg.model || "deepseek-chat";
    this.llmKey = "";
    this.javaList = javas;
    this.mcDirs = mcDirs;
    void this.refreshPreheatStatus();
    if (!this.javaBusy) {
      const pref = (this.settings?.preferred_java ?? "").trim();
      const chosen = pref ? javas.find((j) => j.path === pref) : undefined;
      const best = javas.find((j) => j.major >= 21) ?? javas[0];
      if (chosen) {
        this.javaStatus = `已选用：Java ${chosen.major}（${this.javaSourceLabel(chosen.source)}）`;
      } else if (pref) {
        this.javaStatus = `已选用自定义路径（检测列表里暂未匹配）`;
      } else if (best) {
        this.javaStatus = `自动选择：Java ${best.major}（${this.javaSourceLabel(best.source)}）`;
      } else {
        this.javaStatus = "尚未检测到可用的 Java。点下方按钮即可自动下载并配置。";
      }
    }
  }

  private async setupJava() {
    if (this.javaBusy) return;
    this.javaBusy = true;
    this.javaLog = "";
    this.javaStatus = "正在准备 Java 21…";
    this.unlistenJavaLog?.();
    this.unlistenJavaLog = await listenLogs((line) => {
      this.javaLog = this.javaLog ? `${this.javaLog}\n${line}` : line;
      this.javaStatus = line;
    });
    try {
      const j = await ensureJavaInstall(21);
      this.javaStatus = `配置完成：Java ${j.major}`;
      this.javaLog = [
        this.javaLog,
        `路径：${j.path}`,
        `版本：${j.version_string}`,
        `来源：${j.source}`,
        "下次启动游戏会自动使用它。",
      ]
        .filter(Boolean)
        .join("\n");
      showToast(`Java ${j.major} 已就绪`);
      this.javaList = await detectJava();
    } catch (e) {
      const msg = friendlyError(e);
      this.javaStatus = "配置失败";
      this.javaLog = this.javaLog ? `${this.javaLog}\n${msg}` : msg;
      showToast(`配置失败：${msg}`);
    } finally {
      this.unlistenJavaLog?.();
      this.unlistenJavaLog = null;
      this.javaBusy = false;
    }
  }

  private javaSourceLabel(source: string): string {
    const s = (source || "").trim();
    if (!s) return "未知来源";
    if (s === "JAVA_HOME") return "环境变量 JAVA_HOME";
    if (s === "PATH") return "系统 PATH";
    if (s.startsWith("easymc-runtime:") || s.startsWith("downloaded-temurin")) return "EasyMC 已下载";
    if (s.startsWith("system:")) {
      const kind = s.slice("system:".length);
      if (kind === "linux" || kind === "windows" || kind === "macos") return "系统安装";
      if (kind === "homebrew") return "Homebrew";
      if (kind === "sdkman") return "SDKMAN";
      return `系统（${kind}）`;
    }
    if (s === "preferred") return "已选用";
    return s;
  }

  private preferredJavaPath(): string {
    return (this.settings?.preferred_java ?? "").trim();
  }

  private isPreferredJava(path: string): boolean {
    const pref = this.preferredJavaPath();
    if (!pref) return false;
    return pref === path;
  }

  private async setPreferredJava(path: string | null) {
    if (this.javaBusy) return;
    this.javaBusy = true;
    try {
      this.settings = await updateSettings({ preferred_java: path ?? "" });
      if (path) {
        const hit = this.javaList.find((j) => j.path === path);
        showToast(hit ? `已选用 Java ${hit.major}` : "已选用此 Java");
        this.javaStatus = hit
          ? `已选用：Java ${hit.major}（${this.javaSourceLabel(hit.source)}）`
          : `已选用：${path}`;
      } else {
        showToast("已改回自动选择");
        const best = this.javaList.find((j) => j.major >= 21) ?? this.javaList[0];
        this.javaStatus = best
          ? `自动选择：Java ${best.major}（${this.javaSourceLabel(best.source)}）`
          : "尚未检测到可用的 Java。点下方按钮即可自动下载并配置。";
      }
    } catch (e) {
      showToast(`保存失败：${friendlyError(e)}`);
    } finally {
      this.javaBusy = false;
    }
  }

  private get mode(): AppMode {
    return this.settings?.mode === "agent" ? "agent" : "standard";
  }

  private async refreshPreheatStatus() {
    try {
      this.preheatStatus = await containerPreheatStatus();
    } catch {
      this.preheatStatus = null;
    }
    try {
      this.containerRuntimeProbe = await containerProbe();
    } catch {
      this.containerRuntimeProbe = null;
    }
  }

  private softenPlayerCopy(s: string): string {
    if (!s) return s;
    return friendlyError(s, s)
      .replace(/\bpodman(?:-desktop)?(?:\.io)?\b/gi, "隔离组件")
      .replace(/\bpodman\s+machine\s+\w+/gi, "完成官方安装包里的剩余步骤")
      .replace(/registries\.conf/gi, "加速配置")
      .replace(/https?:\/\/\S*podman\S*/gi, "官方安装包下载页")
      .replace(/\s{2,}/g, " ")
      .trim();
  }

  private isolationStatusLine(): string {
    const enabled = !!this.settings?.container_mode_enabled;
    const switchPart = enabled ? "开关：已开启" : "开关：未开启";
    const probe = this.containerRuntimeProbe;
    const status = (probe?.status || "").toLowerCase();
    let envPart: string;
    if (status === "ready") {
      envPart = "环境：已就绪";
    } else if (this.preheatStatus?.warm) {
      envPart = "环境：已准备好";
    } else if (probe?.message_zh) {
      envPart = "环境：" + this.softenPlayerCopy(probe.message_zh);
    } else if (this.preheatStatus?.message_zh) {
      envPart = "环境：" + this.softenPlayerCopy(this.preheatStatus.message_zh);
    } else if (this.settings?.container_setup_done && !enabled) {
      envPart = "环境：未使用（可随时开启）";
    } else {
      envPart = "环境：尚未检测 · 可点「检查隔离环境」";
    }
    return `${switchPart} · ${envPart}`;
  }

  private containerActionLabel(
    action: NonNullable<SettingsPage["containerBusyAction"]>,
    idle: string,
    busy: string,
  ) {
    if (this.containerBusy && this.containerBusyAction === action) {
      return html`<span class="probe-spin" aria-hidden="true"></span>${busy}`;
    }
    return idle;
  }

  private async runContainerAction(
    action: NonNullable<SettingsPage["containerBusyAction"]>,
    fn: () => Promise<void>,
  ) {
    if (this.containerBusy) return;
    this.containerBusy = true;
    this.containerBusyAction = action;
    try {
      await fn();
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.containerBusy = false;
      this.containerBusyAction = null;
    }
  }

  /** Newbie-friendly mirror labels (no DaoCloud / DockerProxy / BMCLAPI brand noise). */
  private softMirrorLabel(id: string, fallback: string): string {
    const map: Record<string, string> = {
      daocloud: "国内线路 A",
      dockerproxy: "国内线路 B",
      onems: "国内线路 C",
      official: "官方线路（海外）",
      bmclapi: "国内加速",
      bmclapi_alt: "国内加速 · 备用",
    };
    return map[id] || fallback.replace(/DaoCloud|DockerProxy|1ms|BMCLAPI/gi, (m) => {
      const x: Record<string, string> = {
        DaoCloud: "线路 A",
        DockerProxy: "线路 B",
        "1ms": "线路 C",
        BMCLAPI: "国内加速",
      };
      return x[m] || m;
    });
  }

  private async probeDownloads() {
    if (this.downloadProbeBusy) return;
    this.downloadProbeBusy = true;
    try {
      const list = await probeDownloadMirrors();
      this.downloadProbes = list;
      const best = list.find((r) => r.recommended) ?? list.find((r) => r.ok);
      if (best) {
        this.settings = await updateSettings({ download_source: best.download_source });
        showToast(`已选用最快下载源：${this.softMirrorLabel(best.id, best.label_zh)}${best.latency_ms != null ? `（${best.latency_ms} ms）` : ""}`);
      } else {
        showToast("测速完成，但没有可用的下载源");
      }
    } catch (e) {
      showToast(friendlyError(e, "下载源测速失败，请稍后重试"));
    } finally {
      this.downloadProbeBusy = false;
    }
  }

  private async probeRegistries() {
    if (this.registryProbeBusy) return;
    this.registryProbeBusy = true;
    try {
      const list = await probeRegistryMirrors();
      this.registryProbes = list;
      const ok = list.filter((r) => r.ok && r.latency_ms != null);
      const best = ok.length
        ? ok.reduce((a, b) => ((b.latency_ms ?? 1e9) < (a.latency_ms ?? 1e9) ? b : a))
        : list.find((r) => r.recommended) ?? list.find((r) => r.ok);
      if (best) {
        this.settings = await updateSettings({ container_mirror_profile: best.id });
        showToast(`已选用最快加速：${this.softMirrorLabel(best.id, best.label_zh)}${best.latency_ms != null ? `（${best.latency_ms} ms）` : ""}`);
        await this.refreshPreheatStatus();
      } else {
        showToast("测速完成，但没有可用的加速线路");
      }
    } catch (e) {
      showToast(friendlyError(e, "加速源测速失败，请稍后重试"));
    } finally {
      this.registryProbeBusy = false;
    }
  }

  private async saveMem(n: number) {
    if (!n || n < MEM_MIN) {
      showToast(`请至少 ${MEM_MIN} MB`);
      return;
    }
    const clamped = Math.min(MEM_MAX, Math.max(MEM_MIN, Math.round(n / MEM_STEP) * MEM_STEP));
    this.mem = clamped;
    this.settings = await updateSettings({ memory_mb: clamped });
    showToast("已保存");
  }

  render() {
    const tabs = [
      { id: "launch" as const, label: "启动" },
      { id: "multiplayer" as const, label: "联机" },
      { id: "personal" as const, label: "个性化" },
      { id: "other" as const, label: "其他" },
    ];
    return html`
      <h1>设置</h1>
      <p class="sub">先选使用模式，再按需开版本隔离，然后配 Java、内存与助手连接。账户请到「账户」页。</p>
      <div class="md-tabs" role="tablist" aria-label="设置分组">
        <span class="md-tab-ind" aria-hidden="true"></span>
        ${tabs.map(
          (t) => html`<button
            class=${this.section === t.id ? "on" : ""}
            type="button"
            role="tab"
            aria-selected=${this.section === t.id}
            @click=${() => (this.section = t.id)}
          >
            ${t.label}
          </button>`,
        )}
      </div>
      ${this.section === "launch" ? this.renderLaunch() : nothing}
      ${this.section === "multiplayer" ? this.renderMp() : nothing}
      ${this.section === "personal" ? this.renderPersonal() : nothing}
      ${this.section === "other" ? this.renderOther() : nothing}
    `;
  }

  private renderLaunch() {
    const best = this.javaList.find((j) => j.major >= 21) ?? this.javaList[0] ?? null;
    const ready = !!best && best.major >= 21;
    return html`
      <div class="card">
        <h2>使用模式</h2>
        <p>助手模式：侧栏有「助手」，适合不想折腾的玩家；标准模式：经典启动器布局，需要时仍可打开助手。需要时在下方「版本隔离」打开即可。</p>
        <div class="md-segmented" role="radiogroup" aria-label="使用模式">
          <span class="md-tab-ind" aria-hidden="true"></span>
          <button
            class=${this.mode === "agent" ? "on" : ""}
            type="button"
            role="radio"
            aria-checked=${this.mode === "agent"}
            @click=${() => this.setMode("agent")}
          >
            助手模式
          </button>
          <button
            class=${this.mode === "standard" ? "on" : ""}
            type="button"
            role="radio"
            aria-checked=${this.mode === "standard"}
            @click=${() => this.setMode("standard")}
          >
            标准模式
          </button>
        </div>
      </div>
      <div class="card" id="version-isolation">
        <h2>版本隔离（可选）</h2>
        <p>
          把每个版本的模组、配置分开存放，互不打架。更适合「助手模式」；标准模式一般不用开。不装也完全能玩。
        </p>
        <p class="muted" style="margin:0 0 12px" aria-live="polite">
          ${this.isolationStatusLine()}
        </p>
        <div class="row" style="margin-bottom:10px">
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
            <input
              type="checkbox"
              .checked=${!!this.settings?.container_mode_enabled}
              ?disabled=${this.containerBusy}
              @change=${async (e: Event) => {
                const on = (e.target as HTMLInputElement).checked;
                this.settings = await updateSettings({ container_mode_enabled: on });
                showToast(
                  on
                    ? (this.settings?.container_setup_done
                      ? "已开启版本隔离"
                      : "已开启版本隔离 · 可点「配置隔离环境」完成安装")
                    : "已关闭版本隔离",
                );
                await this.refreshPreheatStatus();
              }}
            />
            开启版本隔离
          </label>
        </div>
        <div class="row" style="margin-bottom:10px;align-items:center;gap:12px;flex-wrap:wrap">
          <label for="ctr-mirror">隔离环境下载加速</label>
          <select
            id="ctr-mirror"
            aria-label="隔离环境下载加速"
            ?disabled=${this.containerBusy || this.registryProbeBusy}
            .value=${this.settings?.container_mirror_profile
              || (this.settings?.container_china_mirrors === false ? "official" : "daocloud")}
            @change=${async (e: Event) => {
              const v = (e.target as HTMLSelectElement).value;
              this.settings = await updateSettings({ container_mirror_profile: v });
              const labels: Record<string, string> = {
                daocloud: "国内线路 A",
                dockerproxy: "国内线路 B",
                onems: "国内线路 C",
                official: "官方线路（海外）",
              };
              showToast(`已切换为${labels[v] || v}`);
              await this.refreshPreheatStatus();
            }}
          >
            <option value="daocloud">国内线路 A</option>
            <option value="dockerproxy">国内线路 B</option>
            <option value="onems">国内线路 C</option>
            <option value="official">官方线路（海外）</option>
          </select>
          <button
            class="btn ghost"
            ?disabled=${this.registryProbeBusy || this.containerBusy}
            @click=${() => void this.probeRegistries()}
          >${this.registryProbeBusy
            ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
            : "测速并选用最快"}</button>
        </div>
        ${this.registryProbes.length
          ? html`<ul class="muted" style="margin:0 0 10px;padding-left:18px;line-height:1.6">
              ${this.registryProbes.map(
                (r) => html`<li>
                  ${this.softMirrorLabel(r.id, r.label_zh)}（${r.region_zh}）—
                  ${r.ok && r.latency_ms != null ? `${r.latency_ms} ms` : r.error_zh || "失败"}
                  ${r.recommended ? " · 推荐" : ""}
                </li>`,
              )}
            </ul>`
          : nothing}
        <div class="row" style="margin-bottom:10px">
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
            <input
              type="checkbox"
              .checked=${this.settings?.container_saves_require_propose !== false}
              ?disabled=${this.containerBusy}
              @change=${async (e: Event) => {
                const on = (e.target as HTMLInputElement).checked;
                this.settings = await updateSettings({ container_saves_require_propose: on });
                showToast(on ? "存档修改需确认" : "存档可直接写入（不推荐）");
              }}
            />
            改存档前先问我一声（推荐）
          </label>
        </div>
        <div class="row" style="margin-bottom:10px">
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
            <input
              type="checkbox"
              .checked=${this.settings?.container_auto_preheat !== false}
              ?disabled=${this.containerBusy}
              @change=${async (e: Event) => {
                const on = (e.target as HTMLInputElement).checked;
                this.settings = await updateSettings({ container_auto_preheat: on });
                showToast(on ? "已开启后台提前准备" : "已关闭后台提前准备");
              }}
            />
            后台先准备好，下次开玩更快
          </label>
        </div>
        <div class="row" style="flex-wrap:wrap;gap:8px">
          <button
            class=${this.settings?.container_mode_enabled && !this.settings?.container_setup_done ? "primary" : "tonal"}
            type="button"
            ?disabled=${this.containerBusy}
            title="一键探测并配置隔离环境"
            @click=${() =>
              void this.runContainerAction("setup", async () => {
                this.settings = await updateSettings({
                  container_mode_enabled: true,
                  container_china_mirrors: this.settings?.container_china_mirrors !== false,
                  container_mirror_profile:
                    this.settings?.container_mirror_profile
                    || (this.settings?.container_china_mirrors === false ? "official" : "daocloud"),
                });
                const r = await containerOneClickSetup(
                  this.settings?.container_china_mirrors !== false,
                );
                const ready = (r.probe?.status || "") === "ready";
                this.settings = await updateSettings({
                  container_mode_enabled: true,
                  container_setup_done: ready || (r.pending?.length ?? 0) > 0,
                });
                const pending = (r.pending || [])
                  .map((p) => p.title_zh)
                  .filter(Boolean)
                  .slice(0, 2);
                const tip = this.softenPlayerCopy(
                  (r.messages_zh || []).slice(-1)[0]
                  || (ready ? "隔离环境已就绪" : pending.length ? pending.join("；") : "已记下配置"),
                );
                showToast(tip);
                await this.refreshPreheatStatus();
              })}
          >
            ${this.containerActionLabel("setup", "配置隔离环境", "正在配置…")}
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.containerBusy}
            @click=${() =>
              void this.runContainerAction("probe", async () => {
                const p = await containerProbe();
                this.containerRuntimeProbe = p;
                showToast(this.softenPlayerCopy(p.message_zh));
                await this.refreshPreheatStatus();
              })}
          >
            ${this.containerActionLabel("probe", "检查隔离环境", "检查中…")}
          </button>
          <button
            class="tonal"
            type="button"
            ?disabled=${this.containerBusy || !this.settings?.container_mode_enabled}
            title=${this.settings?.container_mode_enabled ? "现在就准备好" : "先开启版本隔离后再准备"}
            @click=${() =>
              void this.runContainerAction("preheat", async () => {
                const r = await containerPreheat(true);
                showToast(this.softenPlayerCopy(r.messages_zh.join("；") || (r.ok ? "准备完成" : "准备未完成")));
                await this.refreshPreheatStatus();
              })}
          >
            ${this.containerActionLabel("preheat", "现在就准备好", "正在准备…")}
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.containerBusy}
            @click=${() =>
              void this.runContainerAction("registries", async () => {
                const r = await containerWriteRegistries();
                showToast(this.softenPlayerCopy(r.message_zh));
              })}
          >
            ${this.containerActionLabel("registries", "应用加速源配置", "正在应用…")}
          </button>
          <button
            class=${this.settings?.container_mode_enabled ? "tonal" : "ghost"}
            type="button"
            ?disabled=${this.containerBusy || !this.settings?.container_mode_enabled}
            title=${this.settings?.container_mode_enabled ? "修复隔离环境" : "先开启版本隔离后再修复"}
            @click=${() =>
              void this.runContainerAction("repair", async () => {
                const r = await containerRepair({ reprovision: true });
                showToast(this.softenPlayerCopy(r.messages_zh.slice(-2).join("；") || (r.ok ? "修复完成" : "修复未完成")));
                await this.refreshPreheatStatus();
              })}
          >
            ${this.containerActionLabel("repair", "修复隔离环境", "正在修复…")}
          </button>
        </div>
      </div>
      <div class="card">
        <h2>Java 运行环境</h2>
        <p>
          开玩 Minecraft 需要 Java。下面列出本机已检测到的版本；点一行即可设为全局首选（单个版本仍可在版本管理里单独指定）。没有合适版本时，点一次即可自动下载并交给 EasyMC 使用。
        </p>
        <div class=${ready ? "java-ok" : "java-warn"}>${this.javaStatus || (ready && best ? `已就绪：Java ${best.major}` : "尚未检测到可用的 Java")}</div>
        ${this.javaList.length
          ? html`<ul class="java-list" role="listbox" aria-label="已检测到的 Java">
              <li>
                <button
                  type="button"
                  class="java-pick ${!this.preferredJavaPath() ? "on" : ""}"
                  role="option"
                  aria-selected=${!this.preferredJavaPath()}
                  ?disabled=${this.javaBusy}
                  @click=${() => void this.setPreferredJava(null)}
                >
                  <span class="java-pick-title"
                    >自动选择最高版本${!this.preferredJavaPath()
                      ? html`<span class="java-pick-badge">当前</span>`
                      : nothing}</span
                  >
                  <span class="java-pick-meta"
                    >${best
                      ? `当前会用 Java ${best.major}（${this.javaSourceLabel(best.source)}）`
                      : "还没有可用 Java"}</span
                  >
                </button>
              </li>
              ${this.javaList.map((j) => {
                const on = this.isPreferredJava(j.path);
                const low = j.major < 21;
                return html`<li>
                  <button
                    type="button"
                    class="java-pick ${on ? "on" : ""}"
                    role="option"
                    aria-selected=${on}
                    ?disabled=${this.javaBusy}
                    @click=${() => void this.setPreferredJava(j.path)}
                  >
                    <span class="java-pick-title"
                      >Java ${j.major}${on
                        ? html`<span class="java-pick-badge">已选用</span>`
                        : nothing}${low
                        ? html`<span class="java-pick-badge" style="background:color-mix(in oklch, var(--md-sys-color-error) 16%, transparent);color:var(--md-sys-color-error)">偏旧</span>`
                        : nothing}</span
                    >
                    <span class="java-pick-meta"
                      >${this.javaSourceLabel(j.source)} · ${j.version_string}</span
                    >
                    <span class="java-pick-path"><code class="path">${j.path}</code></span>
                  </button>
                </li>`;
              })}
            </ul>`
          : html`<p class="muted" style="margin-top:8px">还没找到本机 Java。可一键下载，或先安装后再点「刷新检测」。</p>`}
        <div class="row" style="margin-top:12px">
          <button
            class="primary"
            type="button"
            ?disabled=${this.javaBusy}
            @click=${() => void this.setupJava()}
          >
            ${this.javaBusy ? "正在配置…" : ready ? "重新检查 / 更新 Java" : "一键配置 Java"}
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.javaBusy}
            @click=${() => void this.refresh()}
          >
            刷新检测
          </button>
          ${this.preferredJavaPath()
            ? html`<button
                class="ghost"
                type="button"
                ?disabled=${this.javaBusy}
                @click=${() => void this.setPreferredJava(null)}
              >
                清除选用
              </button>`
            : nothing}
        </div>
        ${this.javaLog ? html`<div class="java-log" aria-live="polite">${this.javaLog}</div>` : nothing}
      </div>
      
      <div class="card">
        <h2>已有 Minecraft 文件夹</h2>
        <p>
          自动扫描本机已存在的 <code class="path">.minecraft</code> 目录（官方启动器、Flatpak 等），方便沿用里面的版本、存档与设置。
        </p>
        ${this.mcDirs.length
          ? html`<ul class="mc-dirs">
              ${this.mcDirs.map(
                (d) => html`<li>
                  <code class="path">${d.path}</code>
                  <span class="mc-tag">${d.version_count} 个版本 · ${d.source}</span>
                </li>`,
              )}
            </ul>`
          : html`<div class="java-warn">未检测到已有的 Minecraft 文件夹。</div>`}
      </div>
      <div class="card">
        <h2>账户</h2>
        <p>登录方式与当前名字在独立「账户」页管理。</p>
        <div class="row">
          <button
            class="primary"
            type="button"
            @click=${() =>
              this.dispatchEvent(
                new CustomEvent("nav-change", { detail: "accounts", bubbles: true, composed: true }),
              )}
          >
            管理账户
          </button>
        </div>
      </div>
      <div class="card">
        <h2>游戏内存</h2>
        <p>全局默认内存（单个版本可在版本管理里单独改）。</p>
        <div class="slider-row">
          <input
            class="md-slider"
            type="range"
            min=${MEM_MIN}
            max=${MEM_MAX}
            step=${MEM_STEP}
            .value=${String(this.mem)}
            aria-label="游戏内存"
            aria-valuetext="${(this.mem / 1024).toFixed(1)} GB（${this.mem} MB）"
            @input=${(e: Event) => {
              this.mem = Number((e.target as HTMLInputElement).value);
            }}
            @change=${() => void this.saveMem(this.mem)}
          />
          <span class="slider-value" aria-live="polite">${(this.mem / 1024).toFixed(this.mem % 1024 === 0 ? 0 : 1)}<span class="unit">GB</span>
            <span class="unit" style="opacity:.7;margin-left:4px">${this.mem} MB</span></span>
        </div>
        <div class="mem-hints">
          <span>${MEM_MIN / 1024} GB</span>
          <span>推荐 2–8 GB</span>
          <span>${MEM_MAX / 1024} GB</span>
        </div>
      </div>
      <div class="card">
        <h2>下载源</h2>
        <p>游戏文件与版本列表的下载地址。可测速后选用最快源（国内加速）；失败时会自动回退官方源。</p>
        <div class="row" style="margin-bottom:10px;align-items:center;gap:12px;flex-wrap:wrap">
          <select
            aria-label="下载源"
            .value=${this.settings?.download_source === "bmclapi" ? "bmclapi" : "official"}
            @change=${async (e: Event) => {
              const v = (e.target as HTMLSelectElement).value;
              this.settings = await updateSettings({ download_source: v });
              showToast(v === "bmclapi" ? "已切换为国内加速" : "已切换为官方源");
            }}
          >
            <option value="official">官方源</option>
            <option value="bmclapi">国内加速</option>
          </select>
          <button
            class="btn ghost"
            ?disabled=${this.downloadProbeBusy}
            @click=${() => void this.probeDownloads()}
          >${this.downloadProbeBusy
            ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
            : "测速并选用最快"}</button>
        </div>
        ${this.downloadProbes.length
          ? html`<ul class="muted" style="margin:0;padding-left:18px;line-height:1.6">
              ${this.downloadProbes.map(
                (r) => html`<li>
                  ${this.softMirrorLabel(r.id, r.label_zh)}（${r.region_zh}）—
                  ${r.ok && r.latency_ms != null ? `${r.latency_ms} ms` : r.error_zh || "失败"}
                  ${r.recommended ? " · 推荐" : ""}
                </li>`,
              )}
            </ul>`
          : nothing}
      </div>
      <div class="card">
        <h2>助手连接</h2>
        <p>
          想让「助手」页真的会聊天、查崩溃，先在这里填好。密钥只保存在本机，不会上传。标准模式也能配，随时切助手模式即可用。
        </p>
        <div class="muted" style="margin-bottom:8px">
          状态：${
            this.llm?.configured
              ? `已连接 · ${this.llm.model ?? "还没检测到模型"} · ${this.llm.base_url_host ?? ""}`
              : "还没配置（只能用本地规则提示）"
          }
        </div>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">接口地址</span>
          <input
            style="flex:1"
            placeholder="例如 https://api.deepseek.com/v1"
            .value=${this.llmBase}
            @input=${(e: Event) => (this.llmBase = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">密钥</span>
          <input
            style="flex:1"
            type="password"
            autocomplete="off"
            placeholder=${this.llmCfg?.api_key_set ? `已保存 ${this.llmCfg.api_key_hint}（留空则保持）` : "粘贴服务商给你的密钥"}
            .value=${this.llmKey}
            @input=${(e: Event) => (this.llmKey = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">模型名称</span>
          <input
            style="flex:1"
            placeholder="例如 deepseek-chat"
            .value=${this.llmModel}
            @input=${(e: Event) => (this.llmModel = (e.target as HTMLInputElement).value)}
          />
        </label>
        <div class="row" style="margin-top:8px">
          <button
            class="primary"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.saveLlm()}
          >
            保存并启用
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.clearLlm()}
          >
            清除
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.refresh()}
          >
            刷新
          </button>
        </div>
      </div>
      <details class="card">
        <summary>高级</summary>
        <p style="margin-top:12px">数据目录：<code>${this.dataDir || "（还没检测到）"}</code></p>
        <p>
          微软登录客户端：${this.msa?.using_temporary_client_id
            ? "当前使用临时公共客户端，正式发布前会换成 EasyMC 自有。"
            : "自定义客户端。"}
        </p>
        <div class="row">
          <input
            placeholder="微软登录用的应用 ID（一般不用填）"
            .value=${this.msaDraft}
            @input=${(e: Event) => (this.msaDraft = (e.target as HTMLInputElement).value)}
          />
          <button
            class="tonal"
            type="button"
            @click=${async () => {
              this.settings = await updateSettings({ msa_client_id: this.msaDraft.trim() || null });
              this.msa = await msaStatus();
              showToast("已保存");
            }}
          >
            保存
          </button>
          <button
            class="ghost"
            type="button"
            @click=${async () => {
              this.settings = await updateSettings({ clear_msa_client_id: true });
              this.msa = await msaStatus();
              showToast("已恢复默认");
            }}
          >
            恢复默认
          </button>
        </div>
        <h2 style="margin-top:16px">CurseForge 下载密钥（可选）</h2>
        <p>浏览 CurseForge 时需要。Modrinth 不用。</p>
        <div class="muted">状态：${this.cf?.configured ? "已保存" : "未设置"}</div>
        <div class="row">
          <input
            type="password"
            placeholder="粘贴密钥…"
            .value=${this.cfKeyDraft}
            @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)}
          />
          <button
            class="tonal"
            type="button"
            @click=${async () => {
              const r = await setCurseforgeKey(this.cfKeyDraft.trim() || null);
              showToast(r.message);
              this.cf = await curseforgeConfigured();
            }}
          >
            保存
          </button>
          <button
            class="ghost"
            type="button"
            @click=${async () => {
              await setCurseforgeKey(null);
              this.cf = await curseforgeConfigured();
              showToast("已清除");
            }}
          >
            清除
          </button>
        </div>
      </details>



    `;
  }

  private async saveLlm() {
    this.llmBusy = true;
    try {
      this.llm = await agentLlmConfigSave(this.llmBase, this.llmKey, this.llmModel);
      this.llmCfg = await agentLlmConfigGet();
      this.llmKey = "";
      showToast(this.llm.configured ? "云端助手已启用" : "已保存，但仍未就绪");
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.llmBusy = false;
    }
  }

  private async clearLlm() {
    this.llmBusy = true;
    try {
      this.llm = await agentLlmConfigClear();
      this.llmCfg = await agentLlmConfigGet();
      this.llmBase = "";
      this.llmKey = "";
      this.llmModel = "deepseek-chat";
      showToast("已清除云端助手配置");
    } catch (e) {
      showToast(friendlyError(e));
    } finally {
      this.llmBusy = false;
    }
  }

  private renderMp() {
    return html`<div class="card">
      <h2>联机</h2>
      <p>和朋友一起玩的相关选项会在联机功能上线后出现在这里。现在可以先去「联机」页看看预告。</p>
      <button
        class="ghost"
        type="button"
        @click=${() =>
          this.dispatchEvent(
            new CustomEvent("nav-change", { detail: "multiplayer", bubbles: true, composed: true }),
          )}
      >
        查看联机页
      </button>
    </div>`;
  }

  private renderPersonal() {
    return html`<div class="card">
        <h2>主题与动效</h2>
        <p>当前为青绿主题。完整主题定制会在后续版本提供。动效已跟随系统「减少动态效果」设置。</p>
      </div>
      <div class="card">
        <h2>使用模式</h2>
        <p>想换「助手 / 标准」用法？在「启动」分组最上方就能切。</p>
        <button
          class="primary"
          type="button"
          @click=${() => (this.section = "launch")}
        >
          去「启动」切换模式
        </button>
      </div>
      <div class="card">
        <h2>欢迎引导</h2>
        <p>想再走一遍首次设置（模式、账户等），可到「其他」里重新打开。</p>
        <button class="ghost" type="button" @click=${() => (this.section = "other")}>去「其他」</button>
      </div>`;
  }

  private async setMode(m: AppMode) {
    this.settings = await updateSettings({ mode: m });
    this.dispatchEvent(new CustomEvent("mode-change", { detail: m, bubbles: true, composed: true }));
    showToast(m === "agent" ? "已切换到助手模式（需要时打开下方版本隔离）" : "已切换到标准模式");
  }

  private renderOther() {
    return html`
      <div class="card">
        <h2>欢迎引导</h2>
        <p>重新走一遍首次设置（模式与账户）。</p>
        <button
          class="ghost"
          type="button"
          @click=${async () => {
            await updateSettings({ onboarding_complete: false });
            try { localStorage.setItem("easymc.previewOnboarding", "1"); } catch { /* ignore */ }
            showToast("正在打开欢迎引导…");
            this.dispatchEvent(new CustomEvent("onboarding-done", { bubbles: true, composed: true }));
            location.reload();
          }}
        >
          重新显示欢迎引导
        </button>
      </div>
      <div class="card">
        <h2>关于</h2>
        <p>
          EasyMC ${this.settings ? "0.1.20" : ""} · 开源 Minecraft Java 启动器。更多信息见「更多 → 关于」。
        </p>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-settings-page": SettingsPage;
  }
}
