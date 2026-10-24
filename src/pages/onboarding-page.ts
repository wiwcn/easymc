import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  completeOnboarding,
  createOfflineOrThrow,
  detectJava,
  ensureJavaInstall,
  getSettings,
  listenLogs,
  msaStatus,
  updateSettings,
  type JavaInstallDto,
} from "../api";
import { showToast } from "../toast";
import "../components/msa-login";
import type { AppMode, MsaStatusDto, PreferredLogin } from "../types";

type Step = 1 | 2 | 3 | 4;

const MEM_PRESETS = [2048, 4096, 8192] as const;

/** First-run gate: mode → env → login → account → Home. */
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
  @state() private javaReady = false;
  @state() private javaStatus = "";
  @state() private javaLog = "";
  @state() private envBusy = false;
  @state() private envDone = false;
  @state() private dirsReady = false;
  private unlistenJavaLog: (() => void) | null = null;

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
    }
    .wrap { width: min(640px, calc(100vw - 48px)); }
    .brand { font-weight: 700; font-size: var(--easymc-type-display); letter-spacing: -0.02em; margin-bottom: 4px; }
    .steps { display: flex; gap: 6px; margin: 0 0 24px; }
    .steps i { flex: 1; height: 4px; border-radius: 999px; background: var(--md-sys-color-surface-container-highest); }
    .steps i.on { background: var(--md-sys-color-primary); }
    h1 { margin: 0 0 6px; font-size: var(--easymc-type-display); }
    p.sub { margin: 0 0 20px; color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    .options { display: grid; gap: 12px; }
    button.opt {
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      padding: 16px 18px; border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px solid var(--md-sys-color-outline-variant);
      transition: background var(--easymc-dur-press) var(--easymc-ease-out), border-color var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button.opt:hover { background: var(--md-sys-color-surface-container); }
    button.opt.sel { border-color: var(--md-sys-color-primary); background: var(--md-sys-color-primary-container); color: var(--md-sys-color-on-primary-container); }
    button.opt strong { display: block; font-size: var(--easymc-type-title); margin-bottom: 4px; }
    button.opt span { font-size: var(--easymc-type-label); opacity: 0.85; line-height: 1.45; }
    button.opt:focus-visible, .btn:focus-visible, input:focus-visible, select:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    .row { display: flex; gap: 10px; justify-content: space-between; align-items: center; margin-top: 24px; flex-wrap: wrap; }
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
    label { display: flex; flex-direction: column; gap: 6px; font-size: var(--easymc-type-label); font-weight: 500; color: var(--md-sys-color-on-surface-variant); margin-bottom: 12px; }
    input, select {
      padding: 11px 14px; border-radius: 12px;
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
    }
    input.bad { border-color: var(--md-sys-color-error); }
    .err { color: var(--md-sys-color-error); font-size: var(--easymc-type-label); margin: -4px 0 8px; }
    .tip { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    .notice { padding: 12px 14px; border-radius: 12px; background: var(--md-sys-color-surface-container); font-size: var(--easymc-type-body); line-height: 1.5; margin-bottom: 12px; }
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
      font-size: 12px; font-weight: 700;
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
      border-radius: 999px;
      padding: 7px 14px;
      font: inherit;
      cursor: pointer;
    }
    .chips button.on {
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
      font-weight: 600;
    }
    .java-log {
      margin-top: 10px;
      padding: 10px 12px;
      border-radius: 12px;
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
    const s = e instanceof Error ? e.message : String(e);
    if (/invoke|__TAURI|undefined/i.test(s)) return "请在 EasyMC 窗口内完成此操作";
    return s;
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

  private async enterEnvStep() {
    this.step = 2;
    await this.refreshEnv();
  }

  private async refreshEnv() {
    try {
      const [dir, settings, javas] = await Promise.all([
        agentDataDir(),
        getSettings(),
        detectJava(),
      ]);
      this.dataDir = dir.path || "（将在本机自动创建）";
      this.dirsReady = !!dir.path;
      const src = (settings.download_source || "").toLowerCase();
      // Onboarding default: BMCLAPI. Keep official only if already saved as such after a prior prep in this session.
      if (!this.envDone) {
        this.downloadSource = "bmclapi";
      } else if (src === "official" || src === "bmclapi") {
        this.downloadSource = src;
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
      showToast("基础环境已准备好");
    } catch (e) {
      const msg = this.friendly(e);
      this.javaStatus = "准备未完成";
      this.javaLog = this.javaLog ? `${this.javaLog}\n${msg}` : msg;
      showToast(`准备失败：${msg}`);
      this.envDone = false;
    } finally {
      this.unlistenJavaLog?.();
      this.unlistenJavaLog = null;
      this.envBusy = false;
    }
  }

  render() {
    return html`
      <div class="wrap">
        <div class="brand">EasyMC</div>
        <div class="steps" aria-label="第 ${this.step} 步，共 4 步">
          ${[1, 2, 3, 4].map((i) => html`<i class=${i <= this.step ? "on" : ""}></i>`)}
        </div>
        ${this.step === 1
          ? this.renderMode()
          : this.step === 2
            ? this.renderEnv()
            : this.step === 3
              ? this.renderLogin()
              : this.renderAccount()}
      </div>
    `;
  }

  private renderMode() {
    return html`
      <h1>你想怎么用 EasyMC？</h1>
      <p class="sub">之后随时可以在设置里更改。</p>
      <div class="options" role="radiogroup">
        <button class="opt ${this.mode === "agent" ? "sel" : ""}" role="radio" aria-checked=${this.mode === "agent"} @click=${() => (this.mode = "agent")}>
          <strong>Agent 模式</strong>
          <span>助手常驻：游戏崩了帮你看报错、一键修 Java 和内存，适合不想折腾的玩家。</span>
        </button>
        <button class="opt ${this.mode === "standard" ? "sel" : ""}" role="radio" aria-checked=${this.mode === "standard"} @click=${() => (this.mode = "standard")}>
          <strong>标准模式</strong>
          <span>经典启动器布局：选实例、装模组、启动。需要时仍可打开助手。</span>
        </button>
      </div>
      <div class="row">
        <span></span>
        <button class="btn primary" ?disabled=${!this.mode} @click=${() => void this.enterEnvStep()}>下一步</button>
      </div>
    `;
  }

  private renderEnv() {
    return html`
      <h1>准备开玩环境</h1>
      <p class="sub">新电脑也没关系。点一次即可配好 Java、下载源和内存；已装好的会自动跳过。</p>

      <div class="checklist">
        <div class="check ${this.dirsReady ? "ok" : ""}">
          <span class="mark" aria-hidden="true">${this.dirsReady ? "✓" : "1"}</span>
          <div>
            <strong>游戏数据目录</strong>
            <div class="meta">${this.dataDir || "检测中…"}</div>
          </div>
        </div>

        <div class="check ok">
          <span class="mark" aria-hidden="true">✓</span>
          <div>
            <strong>下载源</strong>
            <div class="meta">国内网络建议用加速源；打不开再换官方。</div>
            <div class="chips" role="radiogroup" aria-label="下载源">
              <button type="button" class=${this.downloadSource === "bmclapi" ? "on" : ""} role="radio" aria-checked=${this.downloadSource === "bmclapi"} ?disabled=${this.envBusy} @click=${() => (this.downloadSource = "bmclapi")}>BMCLAPI 加速</button>
              <button type="button" class=${this.downloadSource === "official" ? "on" : ""} role="radio" aria-checked=${this.downloadSource === "official"} ?disabled=${this.envBusy} @click=${() => (this.downloadSource = "official")}>官方源</button>
            </div>
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

      <div class="row">
        <button class="btn ghost" ?disabled=${this.envBusy} @click=${() => (this.step = 1)}>上一步</button>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button class="btn ghost" ?disabled=${this.envBusy} @click=${() => { this.step = 3; showToast("可稍后在「设置 → 启动」里配置 Java"); }}>先跳过</button>
          <button class="btn primary" ?disabled=${this.envBusy || !this.javaReady} @click=${() => (this.step = 3)}>下一步</button>
        </div>
      </div>
    `;
  }

  private renderLogin() {
    return html`
      <h1>选择登录方式</h1>
      <p class="sub">至少需要一个账户才能进入游戏。</p>
      <div class="options" role="radiogroup">
        <button class="opt ${this.login === "microsoft" ? "sel" : ""}" role="radio" aria-checked=${this.login === "microsoft"} @click=${() => this.pickLogin("microsoft")}>
          <strong>正版微软账户</strong>
          <span>使用微软账户登录 Minecraft（正版），可进入正版服务器。</span>
        </button>
        <button class="opt ${this.login === "offline" ? "sel" : ""}" role="radio" aria-checked=${this.login === "offline"} @click=${() => this.pickLogin("offline")}>
          <strong>离线游玩</strong>
          <span>输入想在游戏里显示的名字，马上就能玩单人或离线服务器。</span>
        </button>
        <button class="opt ${this.login === "skin_station" ? "sel" : ""}" role="radio" aria-checked=${this.login === "skin_station"} @click=${() => this.pickLogin("skin_station")}>
          <strong>第三方皮肤站</strong>
          <span>使用皮肤站账号（Authlib / 统一通行证等），显示自定义皮肤。</span>
        </button>
      </div>
      <div class="row">
        <button class="btn ghost" @click=${() => void this.enterEnvStep()}>上一步</button>
        <button class="btn primary" ?disabled=${!this.login} @click=${() => (this.step = 4)}>下一步</button>
      </div>
    `;
  }

  private renderAccount() {
    const back = html`<button class="btn ghost" ?disabled=${this.busy} @click=${() => (this.step = 3)}>上一步</button>`;
    if (this.login === "microsoft") {
      return html`
        <h1>正版登录</h1>
        <p class="sub">用浏览器登录微软账户，EasyMC 不会看到你的密码。</p>
        ${this.msa && !this.msa.configured
          ? html`<div class="notice">正版登录即将可用，可先离线游玩。</div>`
          : html`<easymc-msa-login @msa-done=${(e: CustomEvent<{ username: string }>) => { showToast(`欢迎，${e.detail.username}`); void this.finish(); }}></easymc-msa-login>`}
        <div class="row">
          ${back}
          <button class="btn ghost" ?disabled=${this.busy} @click=${() => { this.login = "offline"; }}>先离线游玩</button>
        </div>
      `;
    }
    if (this.login === "skin_station") {
      return html`
        <h1>皮肤站账号</h1>
        <p class="sub">皮肤站登录即将支持。可以先填好信息，再起一个离线名字进入游戏。</p>
        <label>皮肤站地址<input .value=${this.skinServer} placeholder="例如：https://littleskin.cn" @input=${(e: Event) => (this.skinServer = (e.target as HTMLInputElement).value)} /></label>
        <label>账号（邮箱或用户名）<input .value=${this.skinUser} @input=${(e: Event) => (this.skinUser = (e.target as HTMLInputElement).value)} /></label>
        ${this.skinNote ? html`<div class="notice">${this.skinNote}</div>` : nothing}
        ${this.renderOfflineField("先起一个游戏里显示的名字")}
        <div class="row">
          ${back}
          <div style="display:flex;gap:8px">
            <button class="btn ghost" ?disabled=${this.busy} @click=${() => { this.skinNote = "已记下，皮肤站登录上线后会提醒你完成。"; try { localStorage.setItem("easymc.skinStationDraft", JSON.stringify({ server: this.skinServer, user: this.skinUser })); } catch { /* ignore */ } }}>稍后再说</button>
            <button class="btn primary" ?disabled=${this.busy} @click=${() => this.onOffline()}>进入 EasyMC</button>
          </div>
        </div>
      `;
    }
    return html`
      <h1>离线游玩</h1>
      <p class="sub">输入想在游戏里显示的名字。</p>
      ${this.renderOfflineField("游戏里显示的名字")}
      <div class="row">
        ${back}
        <button class="btn primary" ?disabled=${this.busy} @click=${() => this.onOffline()}>${this.busy ? "正在创建…" : "进入 EasyMC"}</button>
      </div>
    `;
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
