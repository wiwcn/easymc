import { LitElement, css, html } from "lit";
import { customElement, state } from "lit/decorators.js";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { showToast } from "../toast";
import { enterUp } from "../styles/motion";
import { listenLanWorlds } from "../api";
import type { LanWorldDto } from "../types";

/**
 * LAN multiplayer discovery.
 *
 * Minecraft broadcasts "Open to LAN" worlds to 224.0.2.60:4445; the Tauri host
 * listens for a few seconds and returns real worlds. No rooms, accounts, or
 * fake latency numbers are invented here — if nothing is found we say so.
 */
@customElement("easymc-multiplayer-page")
export class MultiplayerPage extends LitElement {
  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
      :host {
        display: block;
        width: 100%;
        box-sizing: border-box;
        padding: var(--easymc-space-5);
      }
      .wrap {
        max-width: 720px;
        margin: 0 auto;
        display: flex;
        flex-direction: column;
        gap: var(--easymc-space-4);
      }
      header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: var(--easymc-space-3);
        flex-wrap: wrap;
      }
      h1 {
        margin: 0 0 var(--easymc-space-1);
      }
      .hint {
        margin: 0;
        color: var(--md-sys-color-on-surface-variant);
        line-height: 1.55;
        font-size: var(--easymc-type-body);
      }
      .toolbar {
        display: flex;
        align-items: center;
        gap: var(--easymc-space-2);
        flex-wrap: wrap;
      }
      .status {
        font-size: var(--easymc-type-label);
        color: var(--md-sys-color-on-surface-variant);
      }
      .list {
        display: flex;
        flex-direction: column;
        gap: var(--easymc-space-2);
      }
      .world {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--easymc-space-3);
        padding: var(--easymc-space-3) var(--easymc-space-4);
        border-radius: var(--md-sys-shape-corner-large);
        background: var(--md-sys-color-surface-container-lowest);
        box-shadow: var(--md-sys-elevation-1);
      }
      .world .name {
        font-weight: var(--easymc-weight-medium);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .world .addr {
        font-family: var(--easymc-font-mono, monospace);
        font-size: var(--easymc-type-label);
        color: var(--md-sys-color-on-surface-variant);
      }
      .world .actions {
        display: flex;
        gap: var(--easymc-space-2);
        flex-shrink: 0;
      }
      .panel {
        padding: var(--easymc-space-5);
        border-radius: var(--md-sys-shape-corner-large);
        background: var(--md-sys-color-surface-container-lowest);
        box-shadow: var(--md-sys-elevation-1);
        color: var(--md-sys-color-on-surface-variant);
        line-height: 1.55;
        font-size: var(--easymc-type-body);
      }
      .panel.error {
        color: var(--md-sys-color-error);
        background: var(--md-sys-color-error-container);
      }
      ol {
        margin: var(--easymc-space-2) 0 0;
        padding-left: var(--easymc-space-4);
        line-height: 1.7;
        font-size: var(--easymc-type-label);
      }
    `,
  ];

  @state() private worlds: LanWorldDto[] = [];
  @state() private scanning = false;
  @state() private scanned = false;
  @state() private error = "";

  protected firstUpdated() {
    enterUp(this.renderRoot.querySelector(".wrap"));
    void this.scan();
  }

  private async scan() {
    if (this.scanning) return;
    this.scanning = true;
    this.error = "";
    try {
      this.worlds = await listenLanWorlds(3);
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.worlds = [];
    } finally {
      this.scanning = false;
      this.scanned = true;
    }
  }

  private async copy(world: LanWorldDto) {
    const address = `${world.host}:${world.port}`;
    try {
      await navigator.clipboard.writeText(address);
      showToast(`已复制 ${address}`);
    } catch {
      showToast(`地址：${address}`, { ms: 4000 });
    }
  }

  private goHome() {
    this.dispatchEvent(
      new CustomEvent("nav-change", { detail: "home", bubbles: true, composed: true }),
    );
  }

  private renderBody() {
    if (this.error) {
      return html`
        <div class="panel error">
          <strong>扫描没成功。</strong><br />
          ${this.error}
        </div>
      `;
    }
    if (this.scanning) {
      return html`<div class="panel">正在听局域网广播…（约 3 秒）</div>`;
    }
    if (this.worlds.length === 0) {
      if (!this.scanned) return html``;
      return html`
        <div class="panel">
          这一轮没发现局域网世界。确认朋友已经在游戏里选择「对局域网开放」，
          并且两台电脑在同一个网络（同一 Wi-Fi / 路由器）下。
        </div>
      `;
    }
    return html`
      <div class="list">
        ${this.worlds.map(
          (w) => html`
            <div class="world">
              <div>
                <div class="name">${w.motd}</div>
                <div class="addr">${w.host}:${w.port}</div>
              </div>
              <div class="actions">
                <button class="ghost" type="button" @click=${() => this.copy(w)}>
                  复制地址
                </button>
              </div>
            </div>
          `,
        )}
      </div>
    `;
  }

  render() {
    return html`
      <div class="wrap">
        <header>
          <div>
            <h1>局域网联机</h1>
            <p class="hint">
              朋友在游戏里点「对局域网开放」后，这个页面会自动发现世界。
              扫描只读取公开的局域网广播，不会上传任何数据。
            </p>
          </div>
          <div class="toolbar">
            <button class="primary" type="button" ?disabled=${this.scanning} @click=${() => this.scan()}>
              ${this.scanning ? "扫描中…" : "重新扫描"}
            </button>
          </div>
        </header>

        ${this.renderBody()}

        <div class="panel">
          <strong>没看到自己的世界？</strong>
          <ol>
            <li>进入存档 → 按 Esc → 选择「对局域网开放」。</li>
            <li>记下聊天栏里显示的端口号，本页会自动发现。</li>
            <li>仍然不显示时，请确认路由器没有开启 AP 隔离 / 访客网络隔离。</li>
          </ol>
        </div>

        <div class="toolbar">
          <button class="ghost" type="button" @click=${() => this.goHome()}>
            先回启动页玩单人
          </button>
        </div>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-multiplayer-page": MultiplayerPage;
  }
}
