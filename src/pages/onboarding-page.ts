import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  completeOnboarding,
  containerOneClickSetup,
  containerProbe,
  createOfflineOrThrow,
  detectJava,
  detectMinecraftDirs,
  ensureJavaInstall,
  getSettings,
  listenLogs,
  msaStatus,
  probeDownloadMirrors,
  probeRegistryMirrors,
  updateSettings,
  type JavaInstallDto,
  type MinecraftDirDto,
} from "../api";
import { clearToasts, showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
import "../components/msa-login";
import type {
  AppMode,
  ContainerRuntimeProbeDto,
  DownloadMirrorProbeDto,
  MsaStatusDto,
  PreferredLogin,
  RegistryMirrorProbeDto,
} from "../types";

type Step = 1 | 2 | 3 | 4 | 5;

const MEM_PRESETS = [2048, 4096, 8192] as const;

/** First-run gate: mode → (container if 助手) → env → login → account → Home. */
@customElement("easymc-onboarding")
export class OnboardingPage extends LitElement {
  @state() private step: Step = 1;
  @state() private mode: AppMode | null = null;
  @state() private login: PreferredLogin | null = null;
  @state() private name = "";
  @state() private nameError = "";
  @state() private busy = false;
  @state() private msa: MsaStatusDto | null = null;
  @state() private skinServer = "";
  @state() private skinUser = "";
  @state() private skinNote = "";

  /** Step 2 — basic environment */
  @state() private dataDir = "";
  @state() private downloadSource: "official" | "bmclapi" = "bmclapi";
  @state() private memoryMb = 2048;
  @state() private javaList: JavaInstallDto[] = [];
  @state() private mcDirs: MinecraftDirDto[] = [];
  @state() private javaReady = false;
  @state() private javaStatus = "";
  @state() private javaLog = "";
  @state() private envBusy = false;
  @state() private envDone = false;
  @state() private dirsReady = false;
  private unlistenJavaLog: (() => void) | null = null;

  /** Step 2 — optional container (ONLY after 助手模式; standard skips to env) */
  @state() private containerWanted: boolean | null = null;
  @state() private containerChina = true;
  @state() private containerProbe: ContainerRuntimeProbeDto | null = null;
  @state() private containerBusy = false;
  @state() private containerLog = "";
  @state() private containerMirrorId = "daocloud";
  @state() private registryProbes: RegistryMirrorProbeDto[] = [];
  @state() private registryProbeBusy = false;
  @state() private downloadProbes: DownloadMirrorProbeDto[] = [];
  @state() private downloadProbeBusy = false;
  @state() private downloadMirrorId = "bmclapi";

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    :host {
      display: grid;
      place-items: center;
      min-height: 100vh;
      width: 100vw;
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
      box-sizing: border-box;
      padding: 24px 0;
    }
    .wrap {
      width: min(640px, calc(100vw - 48px));
      height: min(720px, calc(100vh - 48px));
      display: flex;
      flex-direction: column;
      min-height: 0;
    }
    .chrome-top { flex-shrink: 0; }
    .brand { font-weight: var(--easymc-weight-bold); font-size: var(--easymc-type-display); letter-spacing: -0.02em; margin-bottom: 4px; }
    .steps { display: flex; gap: 6px; margin: 0 0 16px; }
    .steps i { flex: 1; height: 4px; border-radius: var(--md-sys-shape-corner-full); background: var(--md-sys-color-surface-container-highest); }
    .steps i.on { background: var(--md-sys-color-primary); }
    .chrome-top h1 { margin: 0 0 6px; font-size: var(--easymc-type-display); }
    .chrome-top p.sub { margin: 0 0 12px; color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    .scroll {
      flex: 1 1 auto;
      min-height: 0;
      overflow-y: auto;
      overscroll-behavior: contain;
      padding-right: 2px;
    }
    .chrome-foot {
      flex-shrink: 0;
      display: flex;
      gap: 10px;
      justify-content: space-between;
      align-items: center;
      margin-top: 12px;
      padding-top: 12px;
      border-top: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 55%, transparent);
      flex-wrap: wrap;
      min-height: 52px;
    }
    .chrome-foot .foot-right { display: flex; gap: 8px; flex-wrap: wrap; margin-left: auto; }
    h1 { margin: 0 0 6px; font-size: var(--easymc-type-display); }
    p.sub { margin: 0 0 20px; color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    .mirror-list { display: grid; gap: 8px; margin-top: 8px; }
    .mirror-row {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 8px;
      align-items: center;
      padding: 12px 14px;
      border-radius: var(--md-sys-shape-corner-large);
      border: none;
      box-shadow: var(--md-sys-elevation-1);
      background: var(--md-sys-color-surface-container-lowest);
      cursor: pointer;
      text-align: left;
      font: inherit;
      color: inherit;
      transition: background var(--easymc-dur-press) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
    }
    .mirror-row.on {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      outline: 2px solid var(--md-sys-color-primary);
      outline-offset: 0;
      box-shadow: var(--md-sys-elevation-2);
    }
    .mirror-row .lat { font-size: var(--easymc-type-label); opacity: 0.85; white-space: nowrap; }
    .mirror-row .badge {
      display: inline-block;
      margin-left: 6px;
      padding: 2px 8px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-tertiary-container);
      color: var(--md-sys-color-on-tertiary-container);
      font-size: var(--easymc-type-meta);
      font-weight: var(--easymc-weight-medium);
    }
    .probe-btn { margin-top: 8px; display: inline-flex; align-items: center; gap: 8px; }
    .probe-spin {
      width: 14px; height: 14px; border-radius: 50%;
      border: 2px solid color-mix(in oklch, currentColor 35%, transparent);
      border-top-color: currentColor;
      animation: easymc-probe-spin 0.7s linear infinite;
      flex-shrink: 0;
    }
    @keyframes easymc-probe-spin { to { transform: rotate(360deg); } }
    .options { display: grid; gap: 12px; }
    button.opt {
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      padding: 16px 18px; border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-lowest);
      border: none;
      box-shadow: var(--md-sys-elevation-1);
      transition: background var(--easymc-dur-press) var(--easymc-ease-out),
        box-shadow var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.opt:hover { background: var(--md-sys-color-surface-container); box-shadow: var(--md-sys-elevation-2); }
    .skip-cta { font-weight: var(--easymc-weight-medium); text-decoration: underline; text-underline-offset: 3px; }
    button.opt.sel {
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      outline: 2px solid var(--md-sys-color-primary);
      outline-offset: 0;
      box-shadow: var(--md-sys-elevation-2);
    }
    button.opt strong { display: block; font-size: var(--easymc-type-title); margin-bottom: 4px; }
    button.opt span { font-size: var(--easymc-type-label); opacity: 0.85; line-height: 1.45; }
    button.opt:focus-visible, .btn:focus-visible, input:focus-visible, select:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    .row { display: flex; gap: 10px; justify-content: space-between; align-items: center; margin-top: 12px; flex-wrap: wrap; }
    button.btn.primary, .btn.primary {
      border: none;
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      border-radius: var(--md-sys-shape-corner-full);
      font: inherit;
      font-weight: var(--easymc-weight-medium);
      cursor: pointer;
      padding: 11px 22px;
    }
    button.btn.ghost, .btn.ghost {
      border: none;
      background: transparent;
      color: var(--md-sys-color-primary);
      border-radius: var(--md-sys-shape-corner-full);
      font: inherit;
      font-weight: var(--easymc-weight-medium);
      cursor: pointer;
      padding: 11px 22px;
    }
    .btn:disabled { opacity: 0.5; cursor: not-allowed; }
    .btn.primary:disabled {
      opacity: 0.42;
      filter: grayscale(0.25);
      box-shadow: none;
    }
    label { display: flex; flex-direction: column; gap: 6px; font-size: var(--easymc-type-label); font-weight: var(--easymc-weight-medium); color: var(--md-sys-color-on-surface-variant); margin-bottom: 12px; }
    input, select {
      padding: 11px 14px; border-radius: var(--md-sys-shape-corner-medium);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
    }
    input.bad { border-color: var(--md-sys-color-error); }
    .err { color: var(--md-sys-color-error); font-size: var(--easymc-type-label); margin: -4px 0 8px; }
    .tip { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    .notice { padding: 12px 14px; border-radius: var(--md-sys-shape-corner-medium); background: var(--md-sys-color-surface-container); font-size: var(--easymc-type-body); line-height: 1.5; margin-bottom: 12px; }
    .checklist { display: grid; gap: 10px; margin-bottom: 16px; }
    .check {
      display: grid;
      grid-template-columns: 28px 1fr;
      gap: 10px;
      align-items: start;
      padding: 14px 16px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 70%, transparent);
    }
    .check.ok { border-color: color-mix(in oklch, var(--md-sys-color-primary) 45%, transparent); }
    .mark {
      width: 22px; height: 22px; border-radius: 50%;
      display: grid; place-items: center;
      font-size: var(--easymc-type-meta); font-weight: var(--easymc-weight-bold);
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface-variant);
      margin-top: 2px;
    }
    .check.ok .mark {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }
    .check strong { display: block; font-size: var(--easymc-type-title); margin-bottom: 2px; }
    .check .meta { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); line-height: 1.45; word-break: break-all; }
    .chips { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
    .chips button {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: var(--md-sys-shape-corner-full);
      padding: 7px 14px;
      font: inherit;
      cursor: pointer;
    }
    .chips button.on {
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      font-weight: var(--easymc-weight-medium);
    }
    .java-log {
      margin-top: 10px;
      padding: 10px 12px;
      border-radius: var(--md-sys-shape-corner-medium);
      background: var(--md-sys-color-surface-container-lowest);
      font-size: var(--easymc-type-meta);
      color: var(--md-sys-color-on-surface-variant);
      white-space: pre-wrap;
      word-break: break-all;
      max-height: 120px;
      overflow: auto;
    }
    .actions-stack { display: flex; flex-direction: column; gap: 10px; margin-top: 8px; }
    @media (prefers-reduced-motion: reduce) { button.opt { transition: none; } }
  `,
  ];

  disconnectedCallback() {
    this.unlistenJavaLog?.();
    this.unlistenJavaLog = null;
    super.disconnectedCallback();
  }

  private validateName(raw: string): string {
    const n = raw.trim();
    const len = [...n].length;
    if (!n) return "请输入游戏里显示的名字";
    if (len < 2) return "名字至少 2 个字符";
    if (len > 16) return "名字最多 16 个字符";
    if (/[\u0000-\u001f]/.test(n)) return "名字不能包含换行等特殊字符";
    return "";
  }

  private get nameTip(): string {
    const n = this.name.trim();
    if (!n || this.validateName(n)) return "2–16 个字符。离线可用中文名；进多数服务器建议只用英文字母、数字和下划线。";
    if (!/^[A-Za-z0-9_]{3,16}$/.test(n)) return "这个名字离线可以用，但部分服务器只认英文字母、数字和下划线。";
    return "名字可用。";
  }

  private async pickLogin(l: PreferredLogin) {
    this.login = l;
    if (l === "microsoft" && !this.msa) this.msa = await msaStatus();
  }

  private async finish() {
    if (!this.mode) return;
    try {
      await completeOnboarding(this.mode, this.login);
      this.dispatchEvent(new CustomEvent("onboarding-done", { bubbles: true, composed: true }));
    } catch (e) {
      showToast(this.friendly(e));
    }
  }

  private friendly(e: unknown): string {
    return friendlyError(e);
  }

  /** Short top toast so foot CTAs stay clickable; never stick across steps. */
  private softToast(msg: string) {
    showToast(msg, { ms: 1600, placement: "top" });
  }

  private goStep(next: Step) {
    clearToasts();
    this.step = next;
  }

  private async onOffline() {
    const err = this.validateName(this.name);
    this.nameError = err;
    if (err) return;
    this.busy = true;
    try {
      const acc = await createOfflineOrThrow(this.name.trim());
      showToast(`欢迎，${acc.username}`);
      await this.finish();
    } catch (e) {
      this.nameError = this.friendly(e);
    } finally {
      this.busy = false;
    }
  }

  /** Container ask only after 助手模式; 标准模式 jumps straight to env. */
  private async afterModeNext() {
    if (this.mode === "agent") {
      try {
        await updateSettings({ mode: "agent" });
      } catch {
        /* preview */
      }
      await this.enterContainerStep();
      return;
    }
    // Standard: never surface container in onboarding (可稍后在设置里开).
    this.containerWanted = false;
    try {
      await updateSettings({
        mode: "standard",
        container_mode_enabled: false,
        container_setup_done: true,
      });
    } catch {
      /* preview / browser */
    }
    await this.enterEnvStep();
  }

  private async enterContainerStep() {
    this.goStep(2);
    this.containerLog = "";
    try {
      this.containerProbe = await containerProbe();
      const s = await getSettings();
      this.containerChina = s.container_china_mirrors !== false;
      const mid = (s.container_mirror_profile || "").toLowerCase();
      this.containerMirrorId = mid || (this.containerChina ? "daocloud" : "official");
      if (s.container_mode_enabled) this.containerWanted = true;
    } catch (e) {
      this.containerProbe = {
        status: "not_installed",
        message_zh: this.friendly(e),
        platform: "other",
      };
    }
    void this.runRegistryProbe(true);
  }

  private pickContainerMirror(id: string, preferChina: boolean) {
    this.containerMirrorId = id;
    this.containerChina = preferChina;
  }

  /** Real min latency among successful probes (never leave a slower line selected). */
  private pickFastestRegistry(list: Array<{
    id: string; prefer_china?: boolean; ok?: boolean; latency_ms?: number | null; recommended?: boolean;
  }>) {
    const ok = list.filter((r) => r.ok && r.latency_ms != null) as Array<{
      id: string; prefer_china?: boolean; latency_ms: number;
    }>;
    const best = ok.length
      ? ok.reduce((a, b) => (b.latency_ms < a.latency_ms ? b : a))
      : list.find((r) => r.recommended) ?? list.find((r) => r.ok);
    if (best) this.pickContainerMirror(best.id, !!best.prefer_china);
  }

  private async runRegistryProbe(autoPick: boolean) {
    if (this.registryProbeBusy) return;
    this.registryProbeBusy = true;
    try {
      const list = await probeRegistryMirrors();
      this.registryProbes = list;
      if (autoPick) this.pickFastestRegistry(list);
    } catch (e) {
      this.softToast(this.friendly(e));
    } finally {
      this.registryProbeBusy = false;
    }
  }

  private async runDownloadProbe(autoPick: boolean) {
    if (this.downloadProbeBusy) return;
    this.downloadProbeBusy = true;
    try {
      const list = await probeDownloadMirrors();
      this.downloadProbes = list;
      if (autoPick) {
        const best = list.find((r) => r.recommended) ?? list.find((r) => r.ok);
        if (best) {
          this.downloadMirrorId = best.id;
          this.downloadSource = best.download_source === "bmclapi" ? "bmclapi" : "official";
        }
      }
    } catch (e) {
      this.softToast(this.friendly(e));
    } finally {
      this.downloadProbeBusy = false;
    }
  }

  private pickDownloadMirror(id: string, source: string) {
    this.downloadMirrorId = id;
    this.downloadSource = source === "bmclapi" ? "bmclapi" : "official";
  }

  private latencyLabel(ok: boolean, ms?: number | null, err?: string | null): string {
    if (ok && ms != null) return ms < 50 ? "很快" : ms < 150 ? "较快" : `${Math.round(ms)} ms`;
    if (err) {
      const soft = friendlyError(err, "");
      if (!soft || /请在 EasyMC|操作没成功|预览/.test(soft)) return "预览未测";
      return soft.length > 18 ? "暂时测不到" : soft;
    }
    return "未测速";
  }

  /** Newbie-friendly mirror labels (no DaoCloud / DockerProxy / 1ms brand noise). */
  private softMirrorLabel(id: string, fallback: string): string {
    const map: Record<string, string> = {
      daocloud: "国内线路 A",
      dockerproxy: "国内线路 B",
      onems: "国内线路 C",
      official: "官方线路（海外）",
      bmclapi: "国内加速",
      bmclapi_alt: "国内加速 · 备用",
    };
    return map[id] || fallback.replace(/DaoCloud|DockerProxy|1ms/gi, (m) => {
      const x: Record<string, string> = { DaoCloud: "线路 A", DockerProxy: "线路 B", "1ms": "线路 C" };
      return x[m] || m;
    });
  }

  /** From env: 助手 → back to container; 标准 → back to mode. */
  private async backFromEnv() {
    if (this.mode === "agent") {
      await this.enterContainerStep();
      return;
    }
    this.goStep(1);
  }

  private async enterEnvStep() {
    this.goStep(3);
    await this.refreshEnv();
    // Auto-pick fastest download mirror on first enter.
    void this.runDownloadProbe(true);
  }

  private async skipContainer() {
    this.containerWanted = false;
    try {
      await updateSettings({
        container_mode_enabled: false,
        container_setup_done: true,
      });
    } catch {
      /* preview */
    }
    await this.enterEnvStep();
  }

  private async runContainerSetup() {
    if (this.containerBusy) return;
    this.containerBusy = true;
    this.containerLog = "";
    try {
      await updateSettings({
        container_mode_enabled: true,
        container_china_mirrors: this.containerChina,
        container_mirror_profile: this.containerMirrorId,
      });
      const result = await containerOneClickSetup(this.containerChina);
      this.containerProbe = result.probe;
      const lines = [...(result.messages_zh || [])];
      for (const p of result.pending || []) {
        lines.push(`${p.title_zh}：${p.detail_zh}`);
        if (p.command) lines.push(`  $ ${p.command}`);
        if (p.url) lines.push(`  ${p.url}`);
      }
      this.containerLog = lines.filter(Boolean).join("\n");
      const ready = (result.probe?.status || "") === "ready";
      await updateSettings({
        container_mode_enabled: true,
        container_china_mirrors: this.containerChina,
        container_mirror_profile: this.containerMirrorId,
        container_setup_done: ready || (result.pending?.length ?? 0) > 0,
      });
      this.softToast(ready ? "隔离环境已就绪" : "已记下配置，可稍后继续完成安装");
    } catch (e) {
      this.containerLog = this.friendly(e);
      this.softToast(this.friendly(e));
    } finally {
      this.containerBusy = false;
    }
  }

  private async acceptContainerAndContinue() {
    this.containerWanted = true;
    try {
      await updateSettings({
        container_mode_enabled: true,
        container_china_mirrors: this.containerChina,
        container_mirror_profile: this.containerMirrorId,
        container_setup_done: true,
      });
    } catch {
      /* preview */
    }
    await this.enterEnvStep();
  }

  private async refreshEnv() {
    try {
      const [dir, settings, javas, mcDirs] = await Promise.all([
        agentDataDir(),
        getSettings(),
        detectJava(),
        detectMinecraftDirs(),
      ]);
      this.mcDirs = mcDirs;
      this.dataDir = dir.path || "（将在本机自动创建）";
      this.dirsReady = !!dir.path;
      const src = (settings.download_source || "").toLowerCase();
      // Onboarding default: BMCLAPI. Keep official only if already saved as such after a prior prep in this session.
      if (!this.envDone) {
        this.downloadSource = "bmclapi";
        this.downloadMirrorId = "bmclapi";
      } else if (src === "official" || src === "bmclapi") {
        this.downloadSource = src;
        this.downloadMirrorId = src === "bmclapi" ? "bmclapi" : "official";
      }
      const mem = settings.memory_mb ?? 2048;
      this.memoryMb = MEM_PRESETS.includes(mem as (typeof MEM_PRESETS)[number])
        ? (mem as (typeof MEM_PRESETS)[number])
        : mem >= 8192
          ? 8192
          : mem >= 4096
            ? 4096
            : 2048;
      this.javaList = javas;
      const best = javas.find((j) => j.major >= 21) ?? null;
      this.javaReady = !!best;
      this.javaStatus = best
        ? `已找到 Java ${best.major}，无需再下载`
        : "尚未检测到可用的 Java，将自动下载合适版本";
      this.envDone = this.javaReady && this.dirsReady;
    } catch (e) {
      this.javaStatus = this.friendly(e);
      this.dirsReady = false;
      this.javaReady = false;
      this.envDone = false;
    }
  }

  /** One-shot: save download/memory defaults + ensure Java 21 (skip download if present). */
  private async prepareEnv() {
    if (this.envBusy) return;
    this.envBusy = true;
    this.javaLog = "";
    this.unlistenJavaLog?.();
    this.unlistenJavaLog = await listenLogs((line) => {
      this.javaLog = this.javaLog ? `${this.javaLog}\n${line}` : line;
      this.javaStatus = line;
    });
    try {
      // Data dir / defaults: touching settings + data-dir creates/ensures local layout.
      const dir = await agentDataDir();
      this.dataDir = dir.path || this.dataDir;
      this.dirsReady = !!dir.path || this.dirsReady;

      await updateSettings({
        download_source: this.downloadSource,
        memory_mb: this.memoryMb,
      });

      const existing = (await detectJava()).find((j) => j.major >= 21);
      if (existing) {
        this.javaReady = true;
        this.javaStatus = `Java ${existing.major} 已就绪`;
        this.javaLog = [`路径：${existing.path}`, `来源：${existing.source}`, "已跳过下载。"]
          .join("\n");
      } else {
        this.javaStatus = "正在下载并配置 Java 21…";
        const j = await ensureJavaInstall(21);
        this.javaReady = true;
        this.javaStatus = `Java ${j.major} 已就绪`;
        this.javaLog = [
          this.javaLog,
          `路径：${j.path}`,
          `版本：${j.version_string}`,
          `来源：${j.source}`,
        ]
          .filter(Boolean)
          .join("\n");
      }

      this.javaList = await detectJava();
      this.envDone = this.javaReady;
      this.softToast("基础环境已准备好");
    } catch (e) {
      const msg = this.friendly(e);
      this.javaStatus = "准备未完成";
      this.javaLog = this.javaLog ? `${this.javaLog}\n${msg}` : msg;
      this.softToast(`准备失败：${msg}`);
      this.envDone = false;
    } finally {
      this.unlistenJavaLog?.();
      this.unlistenJavaLog = null;
      this.envBusy = false;
    }
  }

  render() {
    const { title, sub, body, footLeft, footRight } = this.stepChrome();
    return html`
      <div class="wrap">
        <div class="chrome-top">
          <div class="brand">EasyMC</div>
          <div class="steps" aria-label=${`第 ${this.progressIndex()} 步，共 ${this.progressTotal()} 步`}>
            ${Array.from({ length: this.progressTotal() }, (_, i) => i + 1).map(
              (i) => html`<i class=${i <= this.progressIndex() ? "on" : ""}></i>`,
            )}
          </div>
          <h1>${title}</h1>
          <p class="sub">${sub}</p>
        </div>
        <div class="scroll">${body}</div>
        <div class="chrome-foot">
          <div>${footLeft}</div>
          <div class="foot-right">${footRight}</div>
        </div>
      </div>
    `;
  }

  /** Visible progress depends on mode: standard skips container. */
  private progressTotal(): number {
    return this.mode === "agent" ? 5 : 4;
  }

  private progressIndex(): number {
    // Logical order: mode(1) → [container(2)] → env(3) → login(4) → account(5)
    if (this.mode === "agent") return this.step;
    // standard: 1→1, 3→2, 4→3, 5→4 (step 2 unused)
    if (this.step <= 1) return 1;
    if (this.step === 3) return 2;
    if (this.step === 4) return 3;
    return 4;
  }

  private stepChrome(): {
    title: string;
    sub: string;
    body: unknown;
    footLeft: unknown;
    footRight: unknown;
  } {
    switch (this.step) {
      case 1:
        return this.chromeMode();
      case 2:
        return this.chromeContainer();
      case 3:
        return this.chromeEnv();
      case 4:
        return this.chromeLogin();
      default:
        return this.chromeAccount();
    }
  }

  private chromeMode() {
    return {
      title: "你想怎么用 EasyMC？",
      sub: "之后随时可以在设置里更改。",
      body: html`
        <div class="options" role="radiogroup">
          <button class="opt ${this.mode === "agent" ? "sel" : ""}" role="radio" aria-checked=${this.mode === "agent"} @click=${() => (this.mode = "agent")}>
            <strong>助手模式</strong>
            <span>助手常驻：游戏崩了帮你看报错、一键修 Java 和内存，适合不想折腾的玩家。</span>
          </button>
          <button class="opt ${this.mode === "standard" ? "sel" : ""}" role="radio" aria-checked=${this.mode === "standard"} @click=${() => (this.mode = "standard")}>
            <strong>标准模式</strong>
            <span>经典启动器布局：选版本、装模组、启动。需要时仍可打开助手。</span>
          </button>
        </div>
      `,
      footLeft: html`${!this.mode
        ? html`<span class="muted" style="font-size:var(--easymc-type-meta)">先点上面选一个用法</span>`
        : html`<span></span>`}`,
      footRight: html`<button class="btn primary" ?disabled=${!this.mode} @click=${() => void this.afterModeNext()}>下一步</button>`,
    };
  }

  private chromeContainer() {
    const probe = this.containerProbe;
    const status = (probe?.status || "").toLowerCase();
    const unsupported = status === "unsupported";
    const ready = status === "ready";
    const mirrors = this.registryProbes.length
      ? this.registryProbes
      : [
          { id: "daocloud", label_zh: "国内线路 A", region_zh: "国内", prefer_china: true, ok: false, latency_ms: null, error_zh: null, recommended: false, sample_image: "" },
          { id: "dockerproxy", label_zh: "国内线路 B", region_zh: "国内", prefer_china: true, ok: false, latency_ms: null, error_zh: null, recommended: false, sample_image: "" },
          { id: "onems", label_zh: "国内线路 C", region_zh: "国内", prefer_china: true, ok: false, latency_ms: null, error_zh: null, recommended: false, sample_image: "" },
          { id: "official", label_zh: "官方线路（海外）", region_zh: "全球", prefer_china: false, ok: false, latency_ms: null, error_zh: null, recommended: false, sample_image: "" },
        ];
    const body = unsupported
      ? html`<div class="notice">${probe?.message_zh || "当前系统不走容器模式。"} 可直接跳过。</div>`
      : html`
          <div class="checklist">
            <div class="check ${ready ? "ok" : ""}">
              <span class="mark" aria-hidden="true">${ready ? "✓" : "·"}</span>
              <div>
                <strong>隔离运行环境</strong>
                <div class="meta">${probe?.message_zh || "检测中…"}</div>
              </div>
            </div>
            <div class="check ok">
              <span class="mark" aria-hidden="true">✓</span>
              <div>
                <strong>下载加速源</strong>
                <div class="meta">国内网络建议选国内加速；测速后可自动选最快，也可自己点。</div>
                <div class="mirror-list" role="radiogroup" aria-label="下载加速源">
                  ${mirrors.map(
                    (m) => html`
                      <button
                        type="button"
                        class="mirror-row ${this.containerMirrorId === m.id ? "on" : ""}"
                        role="radio"
                        aria-checked=${this.containerMirrorId === m.id}
                        ?disabled=${this.containerBusy}
                        @click=${() => this.pickContainerMirror(m.id, !!m.prefer_china)}
                      >
                        <span>
                          ${this.softMirrorLabel(m.id, m.label_zh)}
                          <span class="meta" style="display:inline;margin-left:6px">${m.region_zh}</span>
                          ${m.recommended ? html`<span class="badge">推荐</span>` : nothing}
                        </span>
                        <span class="lat">${this.registryProbeBusy && !m.ok && m.latency_ms == null && !m.error_zh
                          ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
                          : this.latencyLabel(!!m.ok, m.latency_ms, m.error_zh)}</span>
                      </button>
                    `,
                  )}
                </div>
                <button
                  type="button"
                  class="btn ghost probe-btn"
                  ?disabled=${this.registryProbeBusy || this.containerBusy}
                  @click=${() => void this.runRegistryProbe(true)}
                >
                  ${this.registryProbeBusy
                    ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
                    : "测速并选用最快"}
                </button>
              </div>
            </div>
          </div>
          <div class="actions-stack">
            ${this.containerLog ? html`<div class="java-log" aria-live="polite">${this.containerLog}</div>` : nothing}
            ${ready
              ? html`<button class="btn ghost" style="width:100%" ?disabled=${this.containerBusy} @click=${() => void this.runContainerSetup()}>再检查一次</button>`
              : nothing}
          </div>
        `;
    return {
      title: "要用隔离环境吗？（可选）",
      sub: "完全可选。开启后每个版本的模组与存档分开存放；点「配置隔离环境」按提示完成即可。跳过也能继续开玩。",
      body,
      footLeft: html`<button class="btn ghost" ?disabled=${this.containerBusy} @click=${() => this.goStep(1)}>上一步</button>`,
      footRight: html`
        ${ready
          ? html`
              <button class="btn primary" ?disabled=${this.containerBusy} @click=${() => void this.acceptContainerAndContinue()}>已配置，下一步</button>
              <button class="btn ghost" ?disabled=${this.containerBusy} @click=${() => void this.skipContainer()}>暂不使用，继续</button>
            `
          : html`
              <button class="btn primary" ?disabled=${this.containerBusy} @click=${() => void this.runContainerSetup()}>
                ${this.containerBusy ? "正在配置…" : "配置隔离环境"}
              </button>
              <button class="btn ghost" ?disabled=${this.containerBusy} @click=${() => void this.skipContainer()}>暂不使用，继续</button>
            `}
      `,
    };
  }

  private chromeEnv() {
    const dl = this.downloadProbes.length
      ? this.downloadProbes
      : [
          { id: "bmclapi", label_zh: "国内加速", region_zh: "国内", download_source: "bmclapi", ok: false, latency_ms: null, error_zh: null, recommended: false },
          { id: "bmclapi_alt", label_zh: "国内加速 · 备用", region_zh: "国内", download_source: "bmclapi", ok: false, latency_ms: null, error_zh: null, recommended: false },
          { id: "official", label_zh: "官方源", region_zh: "全球", download_source: "official", ok: false, latency_ms: null, error_zh: null, recommended: false },
        ];
    return {
      title: "准备开玩环境",
      sub: "新电脑也没关系。点一次即可配好 Java、下载源和内存；已装好的会自动跳过。",
      body: html`
        <div class="checklist">
          <div class="check ${this.dirsReady ? "ok" : ""}">
            <span class="mark" aria-hidden="true">${this.dirsReady ? "✓" : "1"}</span>
            <div>
              <strong>游戏数据目录</strong>
              <div class="meta">${this.dataDir || "检测中…"}</div>
              ${this.mcDirs.length
                ? html`<div class="meta">检测到已有 Minecraft 文件夹：${this.mcDirs
                    .map((d) => `${d.path}（${d.version_count} 个版本）`)
                    .join("、")}</div>`
                : nothing}
            </div>
          </div>

          <div class="check ok">
            <span class="mark" aria-hidden="true">✓</span>
            <div>
              <strong>下载源</strong>
              <div class="meta">国内网络建议用加速源；可测速后自动选最快，也可手动指定。</div>
              <div class="mirror-list" role="radiogroup" aria-label="下载源">
                ${dl.map(
                  (m) => html`
                    <button
                      type="button"
                      class="mirror-row ${this.downloadMirrorId === m.id ? "on" : ""}"
                      role="radio"
                      aria-checked=${this.downloadMirrorId === m.id}
                      ?disabled=${this.envBusy}
                      @click=${() => this.pickDownloadMirror(m.id, m.download_source)}
                    >
                      <span>
                        ${m.label_zh}
                        <span class="meta" style="display:inline;margin-left:6px">${m.region_zh}</span>
                        ${m.recommended ? html`<span class="badge">推荐·最快</span>` : nothing}
                      </span>
                      <span class="lat">${this.downloadProbeBusy && !m.ok && m.latency_ms == null && !m.error_zh
                        ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
                        : this.latencyLabel(!!m.ok, m.latency_ms, m.error_zh)}</span>
                    </button>
                  `,
                )}
              </div>
              <button
                type="button"
                class="btn ghost probe-btn"
                ?disabled=${this.downloadProbeBusy || this.envBusy}
                @click=${() => void this.runDownloadProbe(true)}
              >
                ${this.downloadProbeBusy
                  ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
                  : "测速并选用最快"}
              </button>
            </div>
          </div>

          <div class="check ok">
            <span class="mark" aria-hidden="true">✓</span>
            <div>
              <strong>游戏内存</strong>
              <div class="meta">先给一个稳妥默认值，以后还能改。</div>
              <div class="chips" role="radiogroup" aria-label="游戏内存">
                ${MEM_PRESETS.map(
                  (m) => html`<button type="button" class=${this.memoryMb === m ? "on" : ""} role="radio" aria-checked=${this.memoryMb === m} ?disabled=${this.envBusy} @click=${() => (this.memoryMb = m)}>${m >= 1024 ? `${m / 1024} GB` : `${m} MB`}</button>`,
                )}
              </div>
            </div>
          </div>

          <div class="check ${this.javaReady ? "ok" : ""}">
            <span class="mark" aria-hidden="true">${this.javaReady ? "✓" : "4"}</span>
            <div>
              <strong>Java 运行环境</strong>
              <div class="meta">${this.javaStatus || "检测中…"}</div>
            </div>
          </div>
        </div>

        <div class="actions-stack">
          <button class="btn primary" style="width:100%" ?disabled=${this.envBusy} @click=${() => void this.prepareEnv()}>
            ${this.envBusy ? "正在准备…" : this.envDone ? "再次检查环境" : "一键准备环境"}
          </button>
          ${this.javaLog ? html`<div class="java-log" aria-live="polite">${this.javaLog}</div>` : nothing}
        </div>
      `,
      footLeft: html`<button class="btn ghost" ?disabled=${this.envBusy} @click=${() => void this.backFromEnv()}>上一步</button>`,
      footRight: html`
        <button class="btn ghost skip-cta" ?disabled=${this.envBusy} @click=${() => { this.goStep(4); this.softToast("可稍后在「设置 → 启动」里配置 Java"); }}>先跳过</button>
        <button class="btn primary" ?disabled=${this.envBusy} @click=${() => this.goStep(4)}>${this.javaReady ? "下一步" : "先继续"}</button>
      `,
    };
  }

  private chromeLogin() {
    return {
      title: "选择登录方式",
      sub: "至少需要一个账户才能进入游戏。",
      body: html`
        <div class="options" role="radiogroup">
          <button class="opt ${this.login === "microsoft" ? "sel" : ""}" role="radio" aria-checked=${this.login === "microsoft"} @click=${() => this.pickLogin("microsoft")}>
            <strong>正版微软账户</strong>
            <span>使用微软账户登录 Minecraft（正版），可进入正版服务器。</span>
          </button>
          <button class="opt ${this.login === "offline" ? "sel" : ""}" role="radio" aria-checked=${this.login === "offline"} @click=${() => this.pickLogin("offline")}>
            <strong>离线游玩</strong>
            <span>马上就能玩单人或离线服务器，下一步再起名字。</span>
          </button>
          <button class="opt ${this.login === "skin_station" ? "sel" : ""}" role="radio" aria-checked=${this.login === "skin_station"} @click=${() => this.pickLogin("skin_station")}>
            <strong>第三方皮肤站</strong>
            <span>用皮肤站账号登录，游戏里显示你的自定义皮肤（完整登录即将支持）。</span>
          </button>
        </div>
      `,
      footLeft: html`<button class="btn ghost" @click=${() => void this.enterEnvStep()}>上一步</button>`,
      footRight: html`<button class="btn primary" ?disabled=${!this.login} @click=${() => this.goStep(5)}>${this.login ? "下一步" : "先选登录方式"}</button>`,
    };
  }

  private chromeAccount() {
    const back = html`<button class="btn ghost" ?disabled=${this.busy} @click=${() => this.goStep(4)}>上一步</button>`;
    if (this.login === "microsoft") {
      return {
        title: "正版登录",
        sub: "用浏览器登录微软账户，EasyMC 不会看到你的密码。",
        body: html`
          ${this.msa && !this.msa.configured
            ? html`<div class="notice">正版登录即将可用，可先离线游玩。</div>`
            : html`<easymc-msa-login @msa-done=${(e: CustomEvent<{ username: string }>) => { showToast(`欢迎，${e.detail.username}`); void this.finish(); }}></easymc-msa-login>`}
        `,
        footLeft: back,
        footRight: html`<button class="btn ghost" ?disabled=${this.busy} @click=${() => { this.login = "offline"; }}>先离线游玩</button>`,
      };
    }
    if (this.login === "skin_station") {
      return {
        title: "皮肤站账号",
        sub: "皮肤站登录即将支持。可以先填好信息，再起一个离线名字进入游戏。",
        body: html`
          <label>皮肤站地址<input .value=${this.skinServer} placeholder="例如：https://littleskin.cn" @input=${(e: Event) => (this.skinServer = (e.target as HTMLInputElement).value)} /></label>
          <label>账号（邮箱或用户名）<input .value=${this.skinUser} @input=${(e: Event) => (this.skinUser = (e.target as HTMLInputElement).value)} /></label>
          ${this.skinNote ? html`<div class="notice">${this.skinNote}</div>` : nothing}
          ${this.renderOfflineField("先起一个游戏里显示的名字")}
        `,
        footLeft: back,
        footRight: html`
          <button class="btn ghost" ?disabled=${this.busy} @click=${() => { this.skinNote = "已记下，皮肤站登录上线后会提醒你完成。"; try { localStorage.setItem("easymc.skinStationDraft", JSON.stringify({ server: this.skinServer, user: this.skinUser })); } catch { /* ignore */ } }}>稍后再说</button>
          <button class="btn primary" ?disabled=${this.busy} @click=${() => this.onOffline()}>进入 EasyMC</button>
        `,
      };
    }
    return {
      title: "离线游玩",
      sub: "输入想在游戏里显示的名字。",
      body: this.renderOfflineField("游戏里显示的名字"),
      footLeft: back,
      footRight: html`<button class="btn primary" ?disabled=${this.busy} @click=${() => this.onOffline()}>${this.busy ? "正在创建…" : "进入 EasyMC"}</button>`,
    };
  }

  private renderOfflineField(label: string) {
    return html`
      <label>${label}
        <input class=${this.nameError ? "bad" : ""} .value=${this.name} maxlength="16" placeholder="例如：Steve"
          @input=${(e: Event) => { this.name = (e.target as HTMLInputElement).value; this.nameError = ""; }}
          @keydown=${(e: KeyboardEvent) => { if (e.key === "Enter") void this.onOffline(); }} />
      </label>
      ${this.nameError ? html`<div class="err" role="alert">${this.nameError}</div>` : html`<div class="tip">${this.nameTip}</div>`}
    `;
  }
}


declare global {
  interface HTMLElementTagNameMap {
    "easymc-onboarding": OnboardingPage;
  }
}
