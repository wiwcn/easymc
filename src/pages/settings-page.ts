import { LitElement, css, html, nothing } from "lit";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  createOfflineOrThrow,
  curseforgeConfigured,
  getSettings,
  listAccounts,
  msaStatus,
  setCurseforgeKey,
  updateSettings,
} from "../api";
import type { AccountDto, AppMode, CurseForgeStatusDto, MsaStatusDto, SettingsDto } from "../types";
import { showToast } from "../toast";
import "../components/msa-login";

@customElement("easymc-settings-page")
export class SettingsPage extends LitElement {
  @state() private settings: SettingsDto | null = null;
  @state() private accounts: AccountDto[] = [];
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private msa: MsaStatusDto | null = null;
  @state() private name = "EasyPlayer";
  @state() private mem = "2048";
  @state() private cfKeyDraft = "";
  @state() private msaDraft = "";
  @state() private dataDir = "";
  @state() private section: "launch" | "multiplayer" | "personal" | "other" = "launch";

  static styles = css`
    :host { display: block; max-width: 720px; }
    h1 { font-size: 1.35rem; font-weight: 650; margin: 8px 0 4px; }
    .sub { color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem; margin-bottom: 14px; }
    .tabs { display: flex; gap: 4px; flex-wrap: wrap; margin-bottom: 14px; }
    .tabs button {
      border: none; background: transparent; cursor: pointer;
      padding: 8px 14px; border-radius: 999px;
      color: var(--md-sys-color-on-surface-variant);
    }
    .tabs button.on {
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container); font-weight: 600;
    }
    .card {
      padding: 16px; margin-bottom: 12px;
      border-radius: var(--md-sys-shape-corner-large);
      background: var(--md-sys-color-surface-container-low);
    }
    .card h2 { margin: 0 0 8px; font-size: 1rem; }
    .card p { margin: 0 0 10px; color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem; }
    .row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
    input, button, select {
      border: 1px solid var(--md-sys-color-outline-variant);
      background: var(--md-sys-color-surface-container-lowest);
      color: var(--md-sys-color-on-surface);
      border-radius: 999px; padding: 8px 14px; font-size: 0.9rem;
    }
    button.primary { border: none; background: var(--md-sys-color-primary); color: var(--md-sys-color-on-primary); cursor: pointer; }
    button.ghost { background: transparent; cursor: pointer; }
    details.card summary { cursor: pointer; font-weight: 600; }
    ul { margin: 8px 0 0; padding-left: 18px; color: var(--md-sys-color-on-surface-variant); font-size: 0.9rem; }
    .muted { color: var(--md-sys-color-on-surface-variant); font-size: 0.85rem; }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.refresh();
  }

  private async refresh() {
    const [s, a, cf, msa, dir] = await Promise.all([
      getSettings(),
      listAccounts(),
      curseforgeConfigured(),
      msaStatus(),
      agentDataDir(),
    ]);
    this.settings = s;
    this.mem = String(s.memory_mb ?? 2048);
    this.accounts = a;
    this.cf = cf;
    this.msa = msa;
    this.dataDir = dir.path;
  }

  private get mode(): AppMode {
    return this.settings?.mode === "agent" ? "agent" : "standard";
  }

  render() {
    const tabs = [
      { id: "launch" as const, label: "启动" },
      { id: "multiplayer" as const, label: "联机" },
      { id: "personal" as const, label: "个性化" },
      { id: "other" as const, label: "其他" },
    ];
    return html`
      <h1>设置</h1>
      <p class="sub">账户、内存与使用偏好。高级选项默认折叠。</p>
      <div class="tabs">
        ${tabs.map((t) => html`<button class=${this.section === t.id ? "on" : ""} type="button" @click=${() => (this.section = t.id)}>${t.label}</button>`)}
      </div>
      ${this.section === "launch" ? this.renderLaunch() : nothing}
      ${this.section === "multiplayer" ? this.renderMp() : nothing}
      ${this.section === "personal" ? this.renderPersonal() : nothing}
      ${this.section === "other" ? this.renderOther() : nothing}
    `;
  }

  private renderLaunch() {
    return html`
      <div class="card">
        <h2>默认账户</h2>
        <p>启动游戏时用哪个名字进游戏。正版服请用微软账户。</p>
        <div class="row">
          <input maxlength="16" placeholder="离线名字" .value=${this.name} @input=${(e: Event) => (this.name = (e.target as HTMLInputElement).value)} />
          <button class="primary" type="button" @click=${async () => {
            try {
              const acc = await createOfflineOrThrow(this.name.trim());
              showToast(`已切换到 ${acc.username}`);
              await this.refresh();
            } catch (e) { showToast(e instanceof Error ? e.message : String(e)); }
          }}>添加离线账户</button>
        </div>
        <ul>${this.accounts.map((a) => html`<li>${a.username} · ${a.kind === "microsoft" ? "正版" : "离线"}</li>`)}</ul>
      </div>
      <div class="card">
        <h2>正版微软账户</h2>
        <p>使用微软账户登录 Minecraft，可进入正版服务器。</p>
        <easymc-msa-login @msa-done=${async (e: CustomEvent) => { showToast(`已登录 ${e.detail.username}`); await this.refresh(); }}></easymc-msa-login>
      </div>
      <div class="card">
        <h2>游戏内存</h2>
        <p>全局默认内存（单个版本可在版本管理里单独改）。</p>
        <div class="row">
          <input type="number" min="512" .value=${this.mem} @input=${(e: Event) => (this.mem = (e.target as HTMLInputElement).value)} />
          <span class="muted">MB</span>
          <button class="primary" type="button" @click=${async () => {
            const n = Number(this.mem);
            if (!n || n < 512) { showToast("请输入至少 512"); return; }
            this.settings = await updateSettings({ memory_mb: n });
            showToast("已保存");
          }}>保存</button>
        </div>
      </div>
      <details class="card">
        <summary>高级</summary>
        <p style="margin-top:12px">数据目录：<code>${this.dataDir || "（未知）"}</code></p>
        <p>微软登录客户端：${this.msa?.using_temporary_client_id ? "当前使用临时公共客户端，正式发布前会换成 EasyMC 自有。" : "自定义客户端。"}</p>
        <div class="row">
          <input placeholder="Azure 应用客户端 ID（可选）" .value=${this.msaDraft} @input=${(e: Event) => (this.msaDraft = (e.target as HTMLInputElement).value)} />
          <button type="button" @click=${async () => { this.settings = await updateSettings({ msa_client_id: this.msaDraft.trim() || null }); this.msa = await msaStatus(); showToast("已保存"); }}>保存</button>
          <button class="ghost" type="button" @click=${async () => { this.settings = await updateSettings({ clear_msa_client_id: true }); this.msa = await msaStatus(); showToast("已恢复默认"); }}>恢复默认</button>
        </div>
        <h2 style="margin-top:16px">CurseForge 密钥（可选）</h2>
        <p>浏览 CurseForge 时需要。Modrinth 不用。</p>
        <div class="muted">状态：${this.cf?.configured ? "已保存" : "未设置"}</div>
        <div class="row">
          <input type="password" placeholder="粘贴密钥…" .value=${this.cfKeyDraft} @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)} />
          <button type="button" @click=${async () => { const r = await setCurseforgeKey(this.cfKeyDraft.trim() || null); showToast(r.message); this.cf = await curseforgeConfigured(); }}>保存</button>
          <button class="ghost" type="button" @click=${async () => { await setCurseforgeKey(null); this.cf = await curseforgeConfigured(); showToast("已清除"); }}>清除</button>
        </div>
      </details>
    `;
  }

  private renderMp() {
    return html`<div class="card">
      <h2>联机</h2>
      <p>和朋友一起玩的相关选项会在联机功能上线后出现在这里。现在可以先去「联机」页看看预告。</p>
      <button class="ghost" type="button" @click=${() => this.dispatchEvent(new CustomEvent("nav-change", { detail: "multiplayer", bubbles: true, composed: true }))}>查看联机页</button>
    </div>`;
  }

  private renderPersonal() {
    return html`<div class="card">
      <h2>使用模式</h2>
      <p>Agent 模式让助手更显眼；标准模式是经典启动器布局。</p>
      <div class="row">
        <button class=${this.mode === "agent" ? "primary" : "ghost"} type="button" @click=${() => this.setMode("agent")}>Agent 模式</button>
        <button class=${this.mode === "standard" ? "primary" : "ghost"} type="button" @click=${() => this.setMode("standard")}>标准模式</button>
      </div>
    </div>
    <div class="card">
      <h2>主题与动效</h2>
      <p>当前为青绿主题。完整主题定制会在后续版本提供。动效已跟随系统「减少动态效果」设置。</p>
    </div>`;
  }

  private async setMode(m: AppMode) {
    this.settings = await updateSettings({ mode: m });
    this.dispatchEvent(new CustomEvent("mode-change", { detail: m, bubbles: true, composed: true }));
    showToast(m === "agent" ? "已切换到 Agent 模式" : "已切换到标准模式");
  }

  private renderOther() {
    return html`
      <div class="card">
        <h2>欢迎引导</h2>
        <p>重新走一遍首次设置（模式与账户）。</p>
        <button class="ghost" type="button" @click=${async () => {
          await updateSettings({ onboarding_complete: false });
          showToast("下次打开将显示欢迎引导");
          this.dispatchEvent(new CustomEvent("onboarding-done", { bubbles: true, composed: true }));
          location.reload();
        }}>重新显示欢迎引导</button>
      </div>
      <div class="card">
        <h2>关于</h2>
        <p>EasyMC ${this.settings ? "0.1.0" : ""} · 开源 Minecraft Java 启动器。更多信息见「更多 → 关于」。</p>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-settings-page": SettingsPage;
  }
}
