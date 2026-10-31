import { LitElement, css, html } from "lit";
import { controlStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import { msaDeviceCode, msaPoll } from "../api";
import type { DeviceCodeDto } from "../types";

type Phase = "idle" | "starting" | "waiting" | "error";

/** 正版登录 (device code). Fires `msa-done` with the AccountDto on success. */
@customElement("easymc-msa-login")
export class MsaLogin extends LitElement {
  @state() private phase: Phase = "idle";
  @state() private info: DeviceCodeDto | null = null;
  @state() private error = "";
  @state() private copied = false;

  static styles = [
    controlStyles,
    css`
    :host { display: block; }
    .code-box {
      display: grid; gap: 12px; padding: 20px; border-radius: var(--md-sys-shape-corner-large, 16px);
      background: var(--md-sys-color-surface-container); text-align: center;
      animation: pop var(--easymc-dur-page, 240ms) var(--easymc-ease-out, ease-out) both;
    }
    @keyframes pop { from { opacity: 0; transform: scale(0.98); } to { opacity: 1; transform: none; } }
    .step { font-size: var(--easymc-type-body); color: var(--md-sys-color-on-surface-variant); line-height: 1.5; }
    a { color: var(--md-sys-color-primary); font-weight: var(--easymc-weight-medium); }
    .code {
      font: 700 2rem/1.2 ui-monospace, "SFMono-Regular", Menlo, monospace; letter-spacing: 0.18em;
      color: var(--md-sys-color-on-primary-container); background: var(--md-sys-color-primary-container);
      padding: 12px 18px; border-radius: var(--md-sys-shape-corner-medium); user-select: all;
    }
    .wait { display: flex; align-items: center; justify-content: center; gap: var(--easymc-space-3); font-size: var(--easymc-type-body); color: var(--md-sys-color-on-surface-variant); }
    .spin { width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--md-sys-color-outline-variant); border-top-color: var(--md-sys-color-primary); animation: spin 900ms linear infinite; }
    @keyframes spin { to { transform: rotate(360deg); } }
    .err { padding: var(--easymc-space-3) var(--easymc-space-4); border-radius: var(--md-sys-shape-corner-medium); background: var(--md-sys-color-error-container); color: var(--md-sys-color-on-error-container); font-size: var(--easymc-type-body); line-height: 1.5; margin-bottom: var(--easymc-space-3); }
    .row { display: flex; gap: 8px; justify-content: center; flex-wrap: wrap; }
    button { padding: 10px 20px; border: none; border-radius: var(--md-sys-shape-corner-full, 999px); font: inherit; font-weight: var(--easymc-weight-medium); cursor: pointer; background: var(--md-sys-color-primary); color: var(--md-sys-color-on-primary); }
    /* ghost → controlStyles; tonal surface override if needed */
    button:disabled { opacity: 0.5; cursor: not-allowed; }
    button:focus-visible { outline: none; box-shadow: var(--easymc-focus-ring); }
    @media (prefers-reduced-motion: reduce) { .code-box { animation: none; } .spin { animation: none; } }
  `,
  ];

  private friendly(e: unknown): string {
    const s = e instanceof Error ? e.message : String(e);
    return /invoke|__TAURI|undefined/i.test(s) ? "请在 EasyMC 窗口内完成此操作" : s;
  }

  async start() {
    this.phase = "starting";
    this.error = "";
    try {
      const info = await msaDeviceCode();
      this.info = info;
      this.phase = "waiting";
      const acc = await msaPoll(info);
      this.phase = "idle";
      this.info = null;
      this.dispatchEvent(new CustomEvent("msa-done", { detail: acc, bubbles: true, composed: true }));
    } catch (e) {
      this.error = this.friendly(e);
      this.phase = "error";
    }
  }

  private async copy() {
    if (!this.info) return;
    try { await navigator.clipboard.writeText(this.info.user_code); this.copied = true; } catch { /* ignore */ }
  }

  render() {
    if (this.phase === "waiting" && this.info) {
      const i = this.info;
      return html`
        <div class="code-box" role="status">
          <div class="step">1. 在浏览器打开 <a href=${i.verification_uri} target="_blank" rel="noreferrer">${i.verification_uri.replace(/^https?:\/\//, "")}</a></div>
          <div class="step">2. 输入下面的代码，用你的微软账户登录</div>
          <div class="code">${i.user_code}</div>
          <div class="row"><button class="ghost" @click=${() => this.copy()}>${this.copied ? "已复制" : "复制代码"}</button></div>
          <div class="wait"><span class="spin" aria-hidden="true"></span>等待你在浏览器里完成登录…（代码 ${Math.round(i.expires_in / 60)} 分钟内有效）</div>
        </div>
      `;
    }
    return html`
      ${this.phase === "error" ? html`<div class="err" role="alert">${this.error}</div>` : null}
      <div class="row">
        <button ?disabled=${this.phase === "starting"} @click=${() => this.start()}>
          ${this.phase === "starting" ? "正在获取登录代码…" : this.phase === "error" ? "重新登录" : "使用微软账户登录"}
        </button>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap { "easymc-msa-login": MsaLogin; }
}
