import { LitElement, css, html } from "lit";
import { customElement } from "lit/decorators.js";

@customElement("easymc-multiplayer-page")
export class MultiplayerPage extends LitElement {
  static styles = css`
    :host { display: block; }
    .box {
      max-width: 480px; margin: 48px auto; text-align: center;
      padding: 36px 28px;
      border-radius: var(--md-sys-shape-corner-extra-large);
      background: var(--md-sys-color-surface-container-low);
      border: 1px dashed var(--md-sys-color-outline-variant);
    }
    h1 { margin: 0 0 8px; font-size: 1.4rem; letter-spacing: -0.02em; }
    p { margin: 0; color: var(--md-sys-color-on-surface-variant); line-height: 1.55; font-size: 0.95rem; }
    .soon {
      display: inline-block; margin-bottom: 12px;
      padding: 4px 12px; border-radius: 999px; font-size: 0.75rem; font-weight: 600;
      background: var(--md-sys-color-tertiary-container);
      color: var(--md-sys-color-on-tertiary-container);
    }
    ul {
      text-align: left; margin: 18px auto 0; padding-left: 20px;
      color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem; line-height: 1.6;
      max-width: 320px;
    }
  `;

  render() {
    return html`
      <div class="box">
        <div class="soon">即将推出</div>
        <h1>和朋友一起玩</h1>
        <p>联机大厅还在准备中。上线后你可以开房间、用邀请码加入，并看到延迟与人数——专心玩游戏，不用自己折腾联机设置。</p>
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
