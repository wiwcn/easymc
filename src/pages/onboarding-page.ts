import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import { completeOnboarding, createOfflineOrThrow, msaStatus, startMicrosoftLogin } from "../api";
import { showToast } from "../toast";
import type { AppMode, MsaStatusDto, PreferredLogin } from "../types";

type Step = 1 | 2 | 3;

/** First-run gate: mode → login method → account → Home. Cannot be skipped. */
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

  static styles = css`
    :host {
      display: grid;
      place-items: center;
      min-height: 100vh;
      width: 100vw;
      background: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
    }
    .wrap { width: min(640px, calc(100vw - 48px)); }
    .brand { font-weight: 700; font-size: 1.4rem; letter-spacing: -0.02em; margin-bottom: 4px; }
    .steps { display: flex; gap: 6px; margin: 0 0 24px; }
    .steps i { flex: 1; height: 4px; border-radius: 999px; background: var(--md-sys-color-surface-container-highest); }
    .steps i.on { background: var(--md-sys-color-primary); }
    h1 { margin: 0 0 6px; font-size: 1.5rem; }
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
    button.opt strong { display: block; font-size: 1rem; margin-bottom: 4px; }
    button.opt span { font-size: 0.88rem; opacity: 0.85; line-height: 1.45; }
    button.opt:focus-visible, .btn:focus-visible, input:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    .row { display: flex; gap: 10px; justify-content: space-between; align-items: center; margin-top: 24px; flex-wrap: wrap; }
    .btn { padding: 11px 22px; border: none; border-radius: var(--md-sys-shape-corner-full); font: inherit; font-weight: 600; cursor: pointer; }
    .btn.primary { background: var(--md-sys-color-primary); color: var(--md-sys-color-on-primary); }
    .btn.ghost { background: transparent; color: var(--md-sys-color-primary); }
    .btn:disabled { opacity: 0.5; cursor: not-allowed; }
    label { display: flex; flex-direction: column; gap: 6px; font-size: 0.88rem; font-weight: 500; color: var(--md-sys-color-on-surface-variant); margin-bottom: 12px; }
    input { padding: 11px 14px; border-radius: 12px; border: 1px solid var(--md-sys-color-outline-variant); font: inherit; background: var(--md-sys-color-surface-container-lowest); color: var(--md-sys-color-on-surface); }
    input.bad { border-color: var(--md-sys-color-error); }
    .err { color: var(--md-sys-color-error); font-size: 0.85rem; margin: -4px 0 8px; }
    .tip { font-size: 0.85rem; color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    .notice { padding: 12px 14px; border-radius: 12px; background: var(--md-sys-color-surface-container); font-size: 0.9rem; line-height: 1.5; margin-bottom: 12px; }
    @media (prefers-reduced-motion: reduce) { button.opt { transition: none; } }
  `;

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

  private async onMsa() {
    this.busy = true;
    try {
      await startMicrosoftLogin();
      await this.finish();
    } catch (e) {
      showToast(this.friendly(e));
    } finally {
      this.busy = false;
    }
  }

  render() {
    return html`
      <div class="wrap">
        <div class="brand">EasyMC</div>
        <div class="steps" aria-label="第 ${this.step} 步，共 3 步">
          ${[1, 2, 3].map((i) => html`<i class=${i <= this.step ? "on" : ""}></i>`)}
        </div>
        ${this.step === 1 ? this.renderMode() : this.step === 2 ? this.renderLogin() : this.renderAccount()}
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
        <button class="btn primary" ?disabled=${!this.mode} @click=${() => (this.step = 2)}>下一步</button>
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
        <button class="btn ghost" @click=${() => (this.step = 1)}>上一步</button>
        <button class="btn primary" ?disabled=${!this.login} @click=${() => (this.step = 3)}>下一步</button>
      </div>
    `;
  }

  private renderAccount() {
    const back = html`<button class="btn ghost" ?disabled=${this.busy} @click=${() => (this.step = 2)}>上一步</button>`;
    if (this.login === "microsoft") {
      return html`
        <h1>正版登录</h1>
        <div class="notice">正版登录即将可用。我们正在接入微软登录流程，现在可以先用离线名字游玩，之后在设置里添加正版账户。</div>
        <div class="row">
          ${back}
          <div style="display:flex;gap:8px">
            <button class="btn ghost" ?disabled=${this.busy} @click=${() => this.onMsa()}>尝试正版登录</button>
            <button class="btn primary" ?disabled=${this.busy} @click=${() => { this.login = "offline"; }}>先离线游玩</button>
          </div>
        </div>
      `;
    }
    if (this.login === "skin_station") {
      return html`
        <h1>皮肤站账号</h1>
        <p class="sub">皮肤站登录即将支持。可以先填好信息，再起一个离线名字进入游戏。</p>
        <label>皮肤站地址<input .value=${this.skinServer} placeholder="例如：https://littleskin.cn" @input=${(e: Event) => (this.skinServer = (e.target as HTMLInputElement).value)} /></label>
        <label>账号（邮箱或用户名）<input .value=${this.skinUser} @input=${(e: Event) => (this.skinUser = (e.target as HTMLInputElement).value)} /></label>
        ${this.skinNote ? html`<div class="notice">${this.skinNote}</div>` : null}
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
