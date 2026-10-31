import { LitElement, css, html } from "lit";
import { customElement } from "lit/decorators.js";
import { controlStyles, pageChromeStyles } from "../styles/controls";

@customElement("easymc-multiplayer-page")
export class MultiplayerPage extends LitElement {
  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
      :host {
        display: flex;
        flex: 1 0 auto;
        min-height: 100%;
        width: 100%;
        align-items: center;
        justify-content: center;
        box-sizing: border-box;
      }
      .box {
        max-width: 28em;
        text-align: center;
        padding: var(--easymc-space-6) var(--easymc-space-5);
        border-radius: var(--md-sys-shape-corner-extra-large);
        background: var(--md-sys-color-surface-container-lowest);
        /* CF-1: soft elev card; optical nudge like EmptyState */
        border: none;
        box-shadow: var(--md-sys-elevation-1);
        transform: translateY(-4%);
      }
      .soon {
        display: inline-block;
        margin-bottom: var(--easymc-space-3);
        padding: var(--easymc-space-1) var(--easymc-space-3);
        border-radius: var(--md-sys-shape-corner-full);
        font-size: var(--easymc-type-meta);
        font-weight: 600;
        background: var(--md-sys-color-tertiary-container);
        color: var(--md-sys-color-on-tertiary-container);
      }
      h1 {
        margin: 0 0 var(--easymc-space-2);
      }
      p {
        margin: 0;
        color: var(--md-sys-color-on-surface-variant);
        line-height: 1.55;
        font-size: var(--easymc-type-body);
      }
      ul {
        text-align: left;
        margin: var(--easymc-space-4) auto 0;
        padding-left: var(--easymc-space-5);
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-label);
        line-height: 1.6;
        max-width: 320px;
      }
      .actions {
        display: flex;
        flex-wrap: wrap;
        gap: var(--easymc-space-2);
        justify-content: center;
        margin-top: var(--easymc-space-5);
      }
    `,
  ];

  render() {
    return html`
      <div class="box">
        <div class="soon">即将推出</div>
        <h1>和朋友一起玩</h1>
        <p>
          联机大厅还在准备中。上线后你可以开房间、用邀请码加入，并看到延迟与人数——专心玩游戏，不用自己折腾联机设置。
        </p>
        <ul>
          <li>创建房间，邀请好友加入</li>
          <li>邀请码一键进房</li>
          <li>查看延迟与人数</li>
        </ul>
        <div class="actions">
          <button
            class="primary"
            type="button"
            @click=${() =>
              this.dispatchEvent(
                new CustomEvent("nav-change", { detail: "home", bubbles: true, composed: true }),
              )}
          >
            先回启动页开单人
          </button>
          <button
            class="ghost"
            type="button"
            @click=${() =>
              window.open(
                "https://github.com/wiwcn/easymc/issues/new?title=%E8%81%94%E6%9C%BA%E9%9C%80%E6%B1%82&body=%E6%88%91%E6%83%B3%E8%A6%81%E7%9A%84%E8%81%94%E6%9C%BA%E6%96%B9%E5%BC%8F%EF%BC%9A%0A",
                "_blank",
              )}
          >
            告诉我你想怎么联机
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
