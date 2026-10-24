import { LitElement, css, html, nothing } from "lit";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  createOfflineOrThrow,
  curseforgeConfigured,
  getSettings,
  listAccountsState,
  msaStatus,
  removeAccount,
  setActiveAccount,
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
  @state() private activeAccountId: string | null = null;
  /** null = list; chooser = add type; offline/microsoft/skin = forms */
  @state() private accountPanel: null | "chooser" | "offline" | "microsoft" | "skin" = null;
  @state() private skinServer = "";
  @state() private skinUser = "";
  @state() private skinNote = "";
  @state() private nameError = "";

  static styles = css`
    :host { display: block; max-width: 720px; }
    h1 { font-size: 1.25rem; font-weight: 650; margin: 0 0 2px; }
    .sub { color: var(--md-sys-color-on-surface-variant); font-size: 0.875rem; margin: 0 0 12px; }
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

    .acct-list { display: grid; gap: 8px; margin-top: 10px; }
    button.acct {
      display: flex; align-items: center; gap: 12px; width: 100%;
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      padding: 12px 14px; border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      border: 1.5px solid color-mix(in oklch, var(--md-sys-color-outline-variant) 45%, transparent);
      transition: border-color 120ms ease, background 120ms ease;
    }
    button.acct:hover { background: var(--md-sys-color-surface-container); }
    button.acct.active {
      border-color: var(--md-sys-color-primary);
      background: var(--md-sys-color-primary-container);
      color: var(--md-sys-color-on-primary-container);
    }
    .skin-ph {
      width: 44px; height: 44px; border-radius: 10px; flex-shrink: 0;
      background:
        linear-gradient(180deg, oklch(0.78 0.06 70) 0 38%, oklch(0.55 0.1 220) 38% 62%, oklch(0.45 0.08 250) 62% 100%);
      box-shadow: var(--md-sys-elevation-1);
      position: relative; overflow: hidden;
    }
    .skin-ph::after {
      content: ""; position: absolute; inset: 8px 14px 10px;
      border-radius: 4px;
      background: oklch(0.92 0.03 90 / 0.35);
    }
    .skin-ph.ms { background: linear-gradient(135deg, oklch(0.55 0.12 250), oklch(0.45 0.1 220)); }
    .skin-ph.skin { background: linear-gradient(135deg, oklch(0.62 0.14 330), oklch(0.5 0.12 300)); }
    .acct-body { flex: 1; min-width: 0; }
    .acct-name { font-weight: 650; font-size: 0.98rem; letter-spacing: -0.01em; }
    .acct-meta { font-size: 0.78rem; opacity: 0.8; margin-top: 2px; display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
    .badge {
      font-size: 0.68rem; font-weight: 700; padding: 2px 8px; border-radius: 999px;
      background: var(--md-sys-color-secondary-container);
      color: var(--md-sys-color-on-secondary-container);
    }
    .badge.ms { background: oklch(0.55 0.12 250 / 0.2); color: oklch(0.4 0.1 250); }
    .badge.off { background: oklch(0.7 0.05 150 / 0.25); color: oklch(0.4 0.08 150); }
    .badge.skin { background: oklch(0.65 0.12 330 / 0.22); color: oklch(0.42 0.12 330); }
    .acct-actions { display: flex; gap: 4px; flex-shrink: 0; }
    .acct-actions button {
      border: none; background: transparent; cursor: pointer;
      padding: 6px 10px; border-radius: 999px; font-size: 0.8rem; font-weight: 600;
      color: var(--md-sys-color-on-surface-variant);
    }
    .acct-actions button:hover { background: var(--md-sys-color-surface-container-highest); }
    .acct-actions button.danger:hover { color: var(--md-sys-color-error); }
    .chooser { display: grid; gap: 10px; margin-top: 8px; }
    button.choice {
      text-align: left; font: inherit; color: inherit; cursor: pointer;
      padding: 14px 16px; border-radius: 16px;
      background: var(--md-sys-color-surface-container-lowest);
      border: 1.5px solid var(--md-sys-color-outline-variant);
    }
    button.choice:hover { border-color: var(--md-sys-color-primary); background: var(--md-sys-color-surface-container); }
    button.choice strong { display: block; font-size: 0.98rem; margin-bottom: 4px; }
    button.choice span { font-size: 0.82rem; color: var(--md-sys-color-on-surface-variant); line-height: 1.4; }
    .form-card {
      margin-top: 10px; padding: 14px; border-radius: 16px;
      background: var(--md-sys-color-surface-container-low);
    }
    .form-card label { display: flex; flex-direction: column; gap: 6px; margin-bottom: 10px; font-size: 0.85rem; color: var(--md-sys-color-on-surface-variant); }
    .form-card input { border-radius: 12px; }
    .err { color: var(--md-sys-color-error); font-size: 0.85rem; margin: -4px 0 8px; }
    .tip { font-size: 0.82rem; color: var(--md-sys-color-on-surface-variant); line-height: 1.45; margin-bottom: 8px; }
    .empty-soft {
      text-align: center; padding: 28px 16px; margin: 8px 0;
      color: var(--md-sys-color-on-surface-variant); font-size: 0.95rem; line-height: 1.55;
    }
    .empty-soft span { font-size: 0.82rem; opacity: 0.85; }
  `;

  connectedCallback() {
    super.connectedCallback();
    void this.refresh();
  }

  private async refresh() {
    const [s, a, cf, msa, dir] = await Promise.all([
      getSettings(),
      listAccountsState(),
      curseforgeConfigured(),
      msaStatus(),
      agentDataDir(),
    ]);
    this.settings = s;
    this.mem = String(s.memory_mb ?? 2048);
    this.accounts = a.accounts;
    this.activeAccountId = a.active_id;
    this.cf = cf;
    this.msa = msa;
    this.dataDir = dir.path;
  }

  private kindLabel(kind: string): string {
    if (kind === "microsoft") return "正版";
    if (kind === "skin_station") return "皮肤站";
    return "离线";
  }

  private kindBadgeClass(kind: string): string {
    if (kind === "microsoft") return "ms";
    if (kind === "skin_station") return "skin";
    return "off";
  }

  private validateName(raw: string): string {
    const n = raw.trim();
    const len = [...n].length;
    if (!n) return "请输入游戏里显示的名字";
    if (len < 2) return "名字至少 2 个字符";
    if (len > 16) return "名字最多 16 个字符";
    return "";
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
        <h2>账户</h2>
        <p>启动游戏时用哪个名字进游戏。点选切换当前账户；正版服请用微软账户。</p>
        ${this.accountPanel === null ? this.renderAccountList() : nothing}
        ${this.accountPanel === "chooser" ? this.renderAddChooser() : nothing}
        ${this.accountPanel === "offline" ? this.renderOfflineForm() : nothing}
        ${this.accountPanel === "microsoft" ? this.renderMsForm() : nothing}
        ${this.accountPanel === "skin" ? this.renderSkinForm() : nothing}
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


  private renderAccountList() {
    return html`
      <div class="acct-list" role="listbox" aria-label="账户列表">
        ${this.accounts.map((a) => {
          const active = a.id === this.activeAccountId;
          return html`<button
            class="acct ${active ? "active" : ""}"
            type="button"
            role="option"
            aria-selected=${active}
            @click=${async () => {
              const st = await setActiveAccount(a.id);
              this.accounts = st.accounts;
              this.activeAccountId = st.active_id;
              showToast(`已切换到 ${a.username}`);
            }}
          >
            <span class="skin-ph ${this.kindBadgeClass(a.kind)}" aria-hidden="true" title="皮肤预览"></span>
            <span class="acct-body">
              <div class="acct-name">${a.username}</div>
              <div class="acct-meta">
                <span class="badge ${this.kindBadgeClass(a.kind)}">${this.kindLabel(a.kind)}</span>
                ${active ? html`<span>当前使用</span>` : nothing}
              </div>
            </span>
            <span class="acct-actions" @click=${(e: Event) => e.stopPropagation()}>
              <button
                class="danger"
                type="button"
                title="移除"
                @click=${async () => {
                  if (!confirm(`移除账户「${a.username}」？`)) return;
                  const st = await removeAccount(a.id);
                  this.accounts = st.accounts;
                  this.activeAccountId = st.active_id;
                  showToast("已移除");
                }}
              >移除</button>
            </span>
          </button>`;
        })}
      </div>
      ${!this.accounts.length
        ? html`<div class="empty-soft">咦？还没有账户呢…<br /><span>点下面加一个，就能开玩啦</span></div>`
        : nothing}
      <div class="row" style="margin-top:12px">
        <button class="primary" type="button" @click=${() => (this.accountPanel = "chooser")}>添加账户</button>
      </div>
    `;
  }

  private renderAddChooser() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.accountPanel = null)}>← 返回列表</button>
      </div>
      <p class="muted" style="margin-top:0">选择登录方式</p>
      <div class="chooser">
        <button class="choice" type="button" @click=${() => (this.accountPanel = "microsoft")}>
          <strong>正版微软账户</strong>
          <span>使用微软账户登录，可进入正版服务器。</span>
        </button>
        <button class="choice" type="button" @click=${() => { this.name = ""; this.nameError = ""; this.accountPanel = "offline"; }}>
          <strong>离线游玩</strong>
          <span>输入游戏里显示的名字，马上就能玩单人或离线服。</span>
        </button>
        <button class="choice" type="button" @click=${() => (this.accountPanel = "skin")}>
          <strong>第三方皮肤站</strong>
          <span>Authlib / 统一通行证等皮肤站账号（即将支持完整登录）。</span>
        </button>
      </div>
    `;
  }

  private renderOfflineForm() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.accountPanel = "chooser")}>← 返回</button>
      </div>
      <div class="form-card">
        <div style="display:flex;justify-content:center;margin-bottom:12px">
          <span class="skin-ph off" style="width:64px;height:64px;border-radius:12px" aria-hidden="true"></span>
        </div>
        <label>游戏用户名
          <input maxlength="16" placeholder="例如：Steve" .value=${this.name}
            @input=${(e: Event) => { this.name = (e.target as HTMLInputElement).value; this.nameError = ""; }}
            @keydown=${(e: KeyboardEvent) => { if (e.key === "Enter") void this.submitOffline(); }} />
        </label>
        ${this.nameError ? html`<div class="err" role="alert">${this.nameError}</div>` : html`<div class="tip">2–16 个字符。离线可用中文名；进多数服务器建议只用英文字母、数字和下划线。</div>`}
        <div class="row">
          <button class="primary" type="button" @click=${() => this.submitOffline()}>添加并使用</button>
        </div>
      </div>
    `;
  }

  private async submitOffline() {
    const err = this.validateName(this.name);
    this.nameError = err;
    if (err) return;
    try {
      const acc = await createOfflineOrThrow(this.name.trim());
      showToast(`已切换到 ${acc.username}`);
      this.accountPanel = null;
      await this.refresh();
    } catch (e) {
      this.nameError = e instanceof Error ? e.message : String(e);
    }
  }

  private renderMsForm() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.accountPanel = "chooser")}>← 返回</button>
      </div>
      <div class="form-card">
        <p class="tip" style="margin-top:0">用浏览器登录微软账户，EasyMC 不会看到你的密码。</p>
        <easymc-msa-login @msa-done=${async (e: CustomEvent) => {
          showToast(`已登录 ${e.detail.username}`);
          this.accountPanel = null;
          await this.refresh();
        }}></easymc-msa-login>
      </div>
    `;
  }

  private renderSkinForm() {
    return html`
      <div class="row" style="margin-bottom:8px">
        <button class="ghost" type="button" @click=${() => (this.accountPanel = "chooser")}>← 返回</button>
      </div>
      <div class="form-card">
        <p class="tip" style="margin-top:0">皮肤站登录即将支持。可先填信息并起一个离线名字进入游戏。</p>
        <label>皮肤站地址<input .value=${this.skinServer} placeholder="例如：https://littleskin.cn" @input=${(e: Event) => (this.skinServer = (e.target as HTMLInputElement).value)} /></label>
        <label>账号（邮箱或用户名）<input .value=${this.skinUser} @input=${(e: Event) => (this.skinUser = (e.target as HTMLInputElement).value)} /></label>
        <label>游戏里显示的名字<input maxlength="16" .value=${this.name} @input=${(e: Event) => { this.name = (e.target as HTMLInputElement).value; this.nameError = ""; }} /></label>
        ${this.nameError ? html`<div class="err">${this.nameError}</div>` : nothing}
        ${this.skinNote ? html`<div class="tip">${this.skinNote}</div>` : nothing}
        <div class="row">
          <button class="ghost" type="button" @click=${() => {
            this.skinNote = "已记下，皮肤站登录上线后会提醒你完成。";
            try { localStorage.setItem("easymc.skinStationDraft", JSON.stringify({ server: this.skinServer, user: this.skinUser })); } catch { /* */ }
          }}>稍后再说</button>
          <button class="primary" type="button" @click=${() => this.submitOffline()}>添加离线名并使用</button>
        </div>
      </div>
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
