import { LitElement, css, html } from "lit";
import { customElement, property } from "lit/decorators.js";
import { open } from "@tauri-apps/plugin-shell";
import type { AppMode } from "../types";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { showToast } from "../toast";
import { APP_VERSION } from "../version";

const REPO_URL = "https://github.com/wiwcn/easymc";
const ISSUES_URL = "https://github.com/wiwcn/easymc/issues/new/choose";

async function openExternal(url: string) {
  try {
    await open(url);
  } catch {
    window.open(url, "_blank", "noopener");
  }
}

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
      button.tile:focus-visible {
        outline: none;
        box-shadow: var(--easymc-focus-ring);
      }
      button.tile strong { font-size: var(--easymc-type-title); }
      button.tile span { font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant); }
      .about {
        margin-top: var(--easymc-space-5); padding: var(--easymc-space-4);
        font-size: var(--easymc-type-label); color: var(--md-sys-color-on-surface-variant);
        line-height: 1.5;
      }
      .about strong { color: var(--md-sys-color-on-surface); font-weight: var(--easymc-weight-medium); }
      a { color: var(--md-sys-color-primary); }
    `,
  ];

  private go(id: string) {
    this.dispatchEvent(new CustomEvent("nav-change", { detail: id, bubbles: true, composed: true }));
  }

  private feedback() {
    void openExternal(ISSUES_URL);
    showToast("已打开 GitHub 反馈页", { ms: 2200 });
  }

  private help() {
    showToast("使用说明即将上线，也可以先问问侧栏「助手」", { ms: 3200 });
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
        <button class="tile surf-outline" type="button" @click=${() => this.go("wardrobe")}>
          <strong>皮肤衣柜</strong>
          <span>三维预览、上传替换，正版 / LittleSkin 导入</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => this.go("instances")}>
          <strong>导出 / 管理版本</strong>
          <span>打开版本管理：导出给好友，或从启动页返回</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => this.feedback()}>
          <strong>反馈</strong>
          <span>在 GitHub 上提 issue，附上日志会更好排查</span>
        </button>
        <button class="tile surf-outline" type="button" @click=${() => this.help()}>
          <strong>帮助</strong>
          <span>使用说明即将上线；也可先问问侧栏「助手」</span>
        </button>
      </div>
      <div class="about surf-outline" id="about">
        <strong>关于 EasyMC</strong><br />
        版本 ${APP_VERSION} · LGPL-3.0-or-later<br />
        开源的 Minecraft Java 启动器，面向玩家、好上手。<br />
        仓库：
        <a href=${REPO_URL} @click=${(e: Event) => { e.preventDefault(); void openExternal(REPO_URL); }}>${REPO_URL}</a><br />
        问题反馈：
        <a href=${ISSUES_URL} @click=${(e: Event) => { e.preventDefault(); void openExternal(ISSUES_URL); }}>GitHub Issues</a>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-more-page": MorePage;
  }
}
