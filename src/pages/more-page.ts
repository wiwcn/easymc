import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import type { AppMode } from "../types";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { showToast } from "../toast";

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
      <p class="sub">分享、帮助与关于。日常启动、下载、助手都在左边栏。</p>
      <div class="grid">
        <button class="tile surf-outline" type="button" @click=${() => this.go("home")}>
          <strong>回启动页</strong>
          <span>选版本、开游戏——最常用的入口</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => this.go("instances")}>
          <strong>导出 / 管理版本</strong>
          <span>打开版本管理：导出给好友，或从启动页返回</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => showToast("仓库公开后可在此反馈。也可发邮件到 feedback@easymc.local（占位）", { ms: 3200 })}>
          <strong>反馈</strong>
          <span>仓库公开后可在此反馈；当前请先记下来，或用下方邮件</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => showToast("帮助文档即将上线。日常问题可先问侧栏「助手」。", { ms: 3200 })}>
          <strong>帮助</strong>
          <span>使用说明即将上线；也可先问问侧栏「助手」</span>
        </button>
      </div>
      <div class="about surf-outline" id="about">
        <strong style="color:var(--md-sys-color-on-surface)">关于 EasyMC</strong><br />
        版本 0.1.13 · MIT 许可<br />
        开源 Minecraft Java Edition 启动器。<br />
        源码仓库公开后可在此查看；反馈占位：
        <a href="mailto:feedback@easymc.local?subject=EasyMC%20反馈">feedback@easymc.local</a>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-more-page": MorePage;
  }
}
