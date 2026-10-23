import { LitElement, css, html } from "lit";
import { customElement, property, state } from "lit/decorators.js";
import { createOffline, listAccounts } from "../api";
import { showToast } from "../toast";
import type { AccountDto } from "../types";

@customElement("easymc-placeholder-page")
export class PlaceholderPage extends LitElement {
  @property() title = "";
  @property() subtitle = "";
  @property() icon = "📦";
  @property({ type: Boolean }) settings = false;

  @state() private accounts: AccountDto[] = [];
  @state() private name = "EasyPlayer";

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
  `;

  connectedCallback() {
    super.connectedCallback();
    if (this.settings) void this.refreshAccounts();
  }

  private async refreshAccounts() {
    this.accounts = await listAccounts();
  }

  private async onCreateOffline() {
    const acc = await createOffline(this.name.trim() || "EasyPlayer");
    if (acc) {
      showToast(`离线账户 ${acc.username}`);
      await this.refreshAccounts();
    } else {
      showToast("需要在 Tauri 应用内操作");
    }
  }

  private async onMsa() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("start_microsoft_login");
    } catch (e) {
      showToast(String(e));
    }
  }

  render() {
    if (this.settings) {
      return html`
        <div class="panel">
          <div class="card">
            <h2>离线账户</h2>
            <p>本地测试无需 Microsoft 登录。多人正版服仍需微软账户。</p>
            <div class="row">
              <input
                .value=${this.name}
                @input=${(e: Event) => (this.name = (e.target as HTMLInputElement).value)}
                placeholder="玩家名"
              />
              <button type="button" @click=${() => this.onCreateOffline()}>创建 / 切换</button>
            </div>
            <ul>
              ${this.accounts.map(
                (a) => html`<li>${a.username} · ${a.kind} · <code>${a.uuid}</code></li>`,
              )}
            </ul>
          </div>
          <div class="card">
            <h2>Microsoft 账户</h2>
            <p>
              需自备 Azure 应用客户端 ID，设置环境变量
              <code>EASYMC_MSA_CLIENT_ID</code>。完整 OAuth → Xbox → Minecraft 换票流程已预留接口，尚未接通。
            </p>
            <div class="row">
              <button class="ghost" type="button" @click=${() => this.onMsa()}>尝试登录（会提示 TODO）</button>
            </div>
          </div>
        </div>
      `;
    }
    return html`
      <div class="center">
        <div class="box">
          <div class="icon">${this.icon}</div>
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
