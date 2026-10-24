import { LitElement, css, html } from "lit";
import { customElement } from "lit/decorators.js";
import { pageChromeStyles } from "../styles/controls";

@customElement("easymc-multiplayer-page")
export class MultiplayerPage extends LitElement {
  static styles = [
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
        background: var(--md-sys-color-surface-container-low);
        /* CF-1: border only (dashed placeholder surface) */
        border: 1px dashed var(--md-sys-color-outline-variant);
        box-shadow: none;
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
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-multiplayer-page": MultiplayerPage;
  }
}
