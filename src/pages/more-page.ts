import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { AppMode } from "../types";
import { controlStyles, pageChromeStyles } from "../styles/controls";

@customElement("easymc-more-page")
export class MorePage extends LitElement {
  @property({ type: String }) mode: AppMode = "standard";

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
      /* Column width comes from the shell (.page > * in app-shell.ts). */
      :host { display: block; }
      .grid { display: grid; gap: var(--easymc-space-3); }
      button.tile {
        display: flex; flex-direction: column; align-items: flex-start; gap: var(--easymc-space-1);
        width: 100%; box-sizing: border-box;
        text-align: left; padding: var(--easymc-space-4) 18px; cursor: pointer;
        color: var(--md-sys-color-on-surface);
        font: inherit;
        transition: background var(--easymc-dur-press) var(--easymc-ease-out);
      }
      button.tile:hover { background: var(--md-sys-color-surface-container); }
      button.tile strong { font-size: var(--easymc-type-title); }
      button.tile span { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); }
      .about {
        margin-top: var(--easymc-space-5); padding: var(--easymc-space-4);
        font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant);
        line-height: 1.5;
      }
      a { color: var(--md-sys-color-primary); }
    `,
  ];

  private go(id: string) {
    this.dispatchEvent(new CustomEvent("nav-change", { detail: id, bubbles: true, composed: true }));
  }

  render() {
    return html`
      <h1>更多</h1>
      <p class="sub">分享、帮助与关于 EasyMC。助手在左侧边栏。</p>
      <div class="grid">
        <button class="tile surf-outline" type="button" @click=${() => this.go("instances")}>
          <strong>分享</strong>
          <span>在版本管理的「导出」里发给好友，或导入好友的分享包</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => window.open("https://github.com/wiwcn/easymc/issues", "_blank")}>
          <strong>反馈</strong>
          <span>在 GitHub 提建议或报告问题</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => window.open("https://github.com/wiwcn/easymc#readme", "_blank")}>
          <strong>帮助</strong>
          <span>使用说明与常见问题</span>
        </button>
      </div>
      <div class="about surf-outline" id="about">
        <strong style="color:var(--md-sys-color-on-surface)">关于 EasyMC</strong><br />
        版本 0.1.0 · MIT 许可<br />
        开源 Minecraft Java Edition 启动器。<br />
        源码：<a href="https://github.com/wiwcn/easymc" target="_blank" rel="noreferrer">github.com/wiwcn/easymc</a>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-more-page": MorePage;
  }
}
