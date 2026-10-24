import { LitElement, css, html } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import {
  createOfflineOrThrow,
  curseforgeConfigured,
  getSettings,
  listAccounts,
  msaStatus,
  setCurseforgeKey,
  updateSettings,
} from "../api";
import { showToast } from "../toast";
import "../components/msa-login";
import type { AccountDto, AppMode, CurseForgeStatusDto, MsaStatusDto } from "../types";

@customElement("easymc-placeholder-page")
export class PlaceholderPage extends LitElement {
  @property() title = "";
  @property() subtitle = "";
  @property() icon = "📦";
  @property({ type: Boolean }) settings = false;

  @state() private accounts: AccountDto[] = [];
  @state() private name = "EasyPlayer";
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private cfKeyDraft = "";
  @state() private mode: AppMode = "standard";
  @state() private msa: MsaStatusDto | null = null;
  @state() private msaDraft = "";

  static styles = css`
    :host {
      display: block;
      min-height: 60vh;
    }
    .center {
      display: grid;
      place-items: center;
      min-height: 50vh;
    }
    .box {
      text-align: center;
      max-width: 480px;
      padding: 40px 28px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px dashed var(--md-sys-color-outline-variant);
    }
    .icon {
      font-size: 3rem;
      margin-bottom: 12px;
    }
    h1 {
      margin: 0 0 8px;
      font-size: 1.5rem;
    }
    p {
      margin: 0;
      color: var(--md-sys-color-on-surface-variant);
      line-height: 1.5;
    }
    .panel {
      max-width: 640px;
      margin: 0 auto;
      display: flex;
      flex-direction: column;
      gap: 16px;
    }
    .card {
      padding: 20px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
      box-shadow: var(--md-sys-elevation-1);
    }
    .card h2 {
      margin: 0 0 8px;
      font-size: 1.1rem;
    }
    .row {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
      margin-top: 12px;
    }
    input {
      flex: 1;
      min-width: 160px;
      padding: 10px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      border: 1px solid var(--md-sys-color-outline-variant);
      font: inherit;
    }
    button {
      padding: 10px 18px;
      border: none;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      font-weight: 600;
      cursor: pointer;
    }
    button.ghost {
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface);
    }
    button {
      transition:
        transform var(--easymc-dur-press) var(--easymc-ease-out),
        opacity var(--easymc-dur-press) var(--easymc-ease-out);
    }
    button:active {
      transform: scale(0.98);
    }
    @media (prefers-reduced-motion: reduce) {
      button:active { transform: none; }
    }
    ul {
      margin: 8px 0 0;
      padding-left: 18px;
      color: var(--md-sys-color-on-surface-variant);
    }
    code {
      font-size: 0.85em;
      background: var(--md-sys-color-surface-container);
      padding: 2px 6px;
      border-radius: 4px;
    }
    a {
      color: var(--md-sys-color-primary);
    }
    summary { cursor: pointer; }
    .muted {
      font-size: 0.85rem;
      color: var(--md-sys-color-on-surface-variant);
      margin-top: 8px;
    }
  `;

  connectedCallback() {
    super.connectedCallback();
    if (this.settings) void this.refreshSettings();
  }

  private async refreshSettings() {
    this.accounts = await listAccounts();
    this.cf = await curseforgeConfigured();
    this.msa = await msaStatus();
    const s = await getSettings();
    this.mode = s.mode === "agent" ? "agent" : "standard";
  }

  private async onMode(m: AppMode) {
    try {
      await updateSettings({ mode: m });
      this.mode = m;
      this.dispatchEvent(new CustomEvent("mode-change", { detail: m, bubbles: true, composed: true }));
      showToast(m === "agent" ? "已切换到 Agent 模式" : "已切换到标准模式");
    } catch {
      showToast("请在 EasyMC 窗口内完成此操作");
    }
  }

  private async onSaveMsaClient(clear = false) {
    try {
      await updateSettings(clear ? { clear_msa_client_id: true } : { msa_client_id: this.msaDraft.trim() });
      this.msaDraft = "";
      this.msa = await msaStatus();
      showToast("已保存");
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    }
  }

  private async onResetOnboarding() {
    if (!window.confirm("下次打开 EasyMC 时会重新显示欢迎引导，确定吗？")) return;
    try {
      await updateSettings({ onboarding_complete: false });
      showToast("已重置，重新打开 EasyMC 后生效");
    } catch {
      showToast("请在 EasyMC 窗口内完成此操作");
    }
  }

  private async refreshAccounts() {
    this.accounts = await listAccounts();
  }

  private async onSaveCfKey() {
    try {
      const res = await setCurseforgeKey(this.cfKeyDraft.trim() || null);
      showToast(res.message);
      this.cfKeyDraft = "";
      this.cf = await curseforgeConfigured();
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    }
  }

  private async onClearCfKey() {
    try {
      const res = await setCurseforgeKey(null);
      showToast(res.message);
      this.cf = await curseforgeConfigured();
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    }
  }

  private async onCreateOffline() {
    try {
      const acc = await createOfflineOrThrow(this.name.trim());
      showToast(`已切换到 ${acc.username}`);
      await this.refreshAccounts();
    } catch (e) {
      const s = e instanceof Error ? e.message : String(e);
      showToast(/invoke|__TAURI|undefined/i.test(s) ? "请在 EasyMC 窗口内完成此操作" : s);
    }
  }

  render() {
    if (this.settings) {
      return html`
        <div class="panel">
          <div class="card">
            <h2>使用模式</h2>
            <p>Agent 模式让助手更显眼；标准模式是经典启动器布局。</p>
            <div class="row">
              <button class=${this.mode === "agent" ? "" : "ghost"} type="button" @click=${() => this.onMode("agent")}>Agent 模式</button>
              <button class=${this.mode === "standard" ? "" : "ghost"} type="button" @click=${() => this.onMode("standard")}>标准模式</button>
            </div>
          </div>
          <div class="card">
            <h2>离线游玩</h2>
            <p>输入想在游戏里显示的名字（2–16 个字符）。进正版服务器需要微软账户。</p>
            <div class="row">
              <input
                .value=${this.name}
                maxlength="16"
                @input=${(e: Event) => (this.name = (e.target as HTMLInputElement).value)}
                placeholder="游戏里显示的名字"
              />
              <button type="button" @click=${() => this.onCreateOffline()}>添加并切换</button>
            </div>
            <ul>
              ${this.accounts.map(
                (a) => html`<li>${a.username} · ${a.kind === "microsoft" ? "正版账户" : "离线账户"}</li>`,
              )}
            </ul>
          </div>
          <div class="card">
            <h2>微软账户（正版）</h2>
            <p>使用微软账户登录 Minecraft（正版），可进入正版服务器。</p>
            <easymc-msa-login @msa-done=${(e: CustomEvent<AccountDto>) => { showToast(`已登录 ${e.detail.username}`); void this.refreshAccounts(); }}></easymc-msa-login>
          </div>
          <details class="card">
            <summary><strong>高级</strong></summary>
            <h2 style="margin-top:12px">CurseForge 密钥（可选）</h2>
            <p>
              若要在商店里浏览 CurseForge 模组，在此粘贴你的密钥（可在
              <a href="https://console.curseforge.com/" target="_blank" rel="noreferrer">CurseForge 控制台</a>
              申请）。Modrinth 无需密钥。
            </p>
            <div class="muted">状态：${this.cf?.configured ? "已保存" : "未设置"}</div>
            <div class="row">
              <input
                type="password"
                autocomplete="off"
                placeholder="粘贴 CurseForge 密钥…"
                .value=${this.cfKeyDraft}
                @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)}
              />
              <button type="button" @click=${() => this.onSaveCfKey()}>保存</button>
              <button class="ghost" type="button" @click=${() => this.onClearCfKey()}>清除</button>
            </div>
            <h2 style="margin-top:16px">微软登录客户端</h2>
            <p>
              ${this.msa?.using_temporary_client_id
                ? html`当前使用<strong>临时</strong>公共客户端（借用 PrismLauncher 开源项目公开的 ID），可能随时失效，正式发布前会替换为 EasyMC 自有客户端。`
                : html`当前使用自定义客户端。`}
              正式发布前会换成 EasyMC 自己的登录客户端。
            </p>
            <div class="row">
              <input
                placeholder="Azure 应用客户端 ID（可选）"
                .value=${this.msaDraft}
                @input=${(e: Event) => (this.msaDraft = (e.target as HTMLInputElement).value)}
              />
              <button type="button" @click=${() => this.onSaveMsaClient()}>保存</button>
              <button class="ghost" type="button" @click=${() => this.onSaveMsaClient(true)}>恢复默认</button>
            </div>
            <h2 style="margin-top:16px">欢迎引导</h2>
            <div class="row">
              <button class="ghost" type="button" @click=${() => this.onResetOnboarding()}>重新显示欢迎引导</button>
            </div>
          </details>
        </div>
      `;
    }
    return html`
      <div class="center">
        <div class="box">
          ${this.icon ? html`<div class="icon">${this.icon}</div>` : null}
          <h1>${this.title}</h1>
          <p>${this.subtitle}</p>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-placeholder-page": PlaceholderPage;
  }
}
