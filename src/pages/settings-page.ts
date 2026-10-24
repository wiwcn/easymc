import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  agentLlmStatus,
  curseforgeConfigured,
  getSettings,
  msaStatus,
  setCurseforgeKey,
  updateSettings,
} from "../api";
import type { AppMode, CurseForgeStatusDto, LlmStatusDto, MsaStatusDto, SettingsDto } from "../types";
import { showToast } from "../toast";

const MEM_MIN = 512;
const MEM_MAX = 16384;
const MEM_STEP = 256;

@customElement("easymc-settings-page")
export class SettingsPage extends LitElement {
  @state() private settings: SettingsDto | null = null;
  @state() private cf: CurseForgeStatusDto | null = null;
  @state() private msa: MsaStatusDto | null = null;
  @state() private mem = 2048;
  @state() private cfKeyDraft = "";
  @state() private msaDraft = "";
  @state() private dataDir = "";
  @state() private llm: LlmStatusDto | null = null;
  @state() private section: "launch" | "multiplayer" | "personal" | "other" = "launch";

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
      :host {
        display: block;
        max-width: 720px;
      }
      h1 {
        font-size: var(--easymc-type-title);
        font-weight: var(--easymc-weight-bold);
        margin: 0 0 2px;
      }
      .sub {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-meta);
        margin: 0 0 var(--easymc-space-4);
      }
      .tabs {
        display: flex;
        gap: 4px;
        flex-wrap: wrap;
        margin-bottom: 14px;
      }
      .tabs button {
        border: none;
        background: transparent;
        cursor: pointer;
        padding: 8px 14px;
        border-radius: 999px;
        color: var(--md-sys-color-on-surface-variant);
        font: inherit;
      }
      .tabs button.on {
        background: var(--md-sys-color-secondary-container);
        color: var(--md-sys-color-on-secondary-container);
        font-weight: 600;
      }
      .card {
        padding: var(--easymc-space-4);
        margin-bottom: 12px;
        border-radius: var(--md-sys-shape-corner-large);
        background: var(--md-sys-color-surface-container-low);
      }
      .card h2 {
        margin: 0 0 8px;
        font-size: var(--easymc-type-title);
      }
      .card p {
        margin: 0 0 10px;
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-body);
      }
      .row {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        align-items: center;
      }
      input:not([type="range"]),
      select {
        border: 1px solid var(--md-sys-color-outline-variant);
        background: var(--md-sys-color-surface-container-lowest);
        color: var(--md-sys-color-on-surface);
        border-radius: 999px;
        padding: 8px 14px;
        font-size: var(--easymc-type-body);
      }
      details.card summary {
        cursor: pointer;
        font-weight: 600;
      }
      .muted {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-label);
      }
      .mem-hints {
        display: flex;
        justify-content: space-between;
        margin-top: 4px;
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
      }
    `,
  ];

  connectedCallback() {
    super.connectedCallback();
    void this.refresh();
  }

  private async refresh() {
    const [s, cf, msa, dir, llm] = await Promise.all([
      getSettings(),
      curseforgeConfigured(),
      msaStatus(),
      agentDataDir(),
      agentLlmStatus(),
    ]);
    this.settings = s;
    this.mem = Math.min(MEM_MAX, Math.max(MEM_MIN, s.memory_mb ?? 2048));
    this.cf = cf;
    this.msa = msa;
    this.dataDir = dir.path;
    this.llm = llm;
  }

  private get mode(): AppMode {
    return this.settings?.mode === "agent" ? "agent" : "standard";
  }

  private async saveMem(n: number) {
    if (!n || n < MEM_MIN) {
      showToast(`请至少 ${MEM_MIN} MB`);
      return;
    }
    const clamped = Math.min(MEM_MAX, Math.max(MEM_MIN, Math.round(n / MEM_STEP) * MEM_STEP));
    this.mem = clamped;
    this.settings = await updateSettings({ memory_mb: clamped });
    showToast("已保存");
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
      <p class="sub">内存与使用偏好。账户请到「账户」页管理。高级选项默认折叠。</p>
      <div class="tabs">
        ${tabs.map(
          (t) => html`<button
            class=${this.section === t.id ? "on" : ""}
            type="button"
            @click=${() => (this.section = t.id)}
          >
            ${t.label}
          </button>`,
        )}
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
        <p>登录方式与当前名字在独立「账户」页管理。</p>
        <div class="row">
          <button
            class="primary"
            type="button"
            @click=${() =>
              this.dispatchEvent(
                new CustomEvent("nav-change", { detail: "accounts", bubbles: true, composed: true }),
              )}
          >
            管理账户
          </button>
        </div>
      </div>
      <div class="card">
        <h2>游戏内存</h2>
        <p>全局默认内存（单个版本可在版本管理里单独改）。</p>
        <div class="slider-row">
          <input
            class="md-slider"
            type="range"
            min=${MEM_MIN}
            max=${MEM_MAX}
            step=${MEM_STEP}
            .value=${String(this.mem)}
            aria-label="游戏内存（兆字节）"
            aria-valuetext="${this.mem} 兆字节"
            @input=${(e: Event) => {
              this.mem = Number((e.target as HTMLInputElement).value);
            }}
            @change=${() => void this.saveMem(this.mem)}
          />
          <span class="slider-value" aria-live="polite">${this.mem}<span class="unit">MB</span></span>
        </div>
        <div class="mem-hints">
          <span>${MEM_MIN}</span>
          <span>推荐 2048–8192</span>
          <span>${MEM_MAX}</span>
        </div>
      </div>
      <details class="card">
        <summary>高级</summary>
        <p style="margin-top:12px">数据目录：<code>${this.dataDir || "（未知）"}</code></p>
        <p>
          微软登录客户端：${this.msa?.using_temporary_client_id
            ? "当前使用临时公共客户端，正式发布前会换成 EasyMC 自有。"
            : "自定义客户端。"}
        </p>
        <div class="row">
          <input
            placeholder="Azure 应用客户端 ID（可选）"
            .value=${this.msaDraft}
            @input=${(e: Event) => (this.msaDraft = (e.target as HTMLInputElement).value)}
          />
          <button
            class="tonal"
            type="button"
            @click=${async () => {
              this.settings = await updateSettings({ msa_client_id: this.msaDraft.trim() || null });
              this.msa = await msaStatus();
              showToast("已保存");
            }}
          >
            保存
          </button>
          <button
            class="ghost"
            type="button"
            @click=${async () => {
              this.settings = await updateSettings({ clear_msa_client_id: true });
              this.msa = await msaStatus();
              showToast("已恢复默认");
            }}
          >
            恢复默认
          </button>
        </div>
        <h2 style="margin-top:16px">CurseForge 密钥（可选）</h2>
        <p>浏览 CurseForge 时需要。Modrinth 不用。</p>
        <div class="muted">状态：${this.cf?.configured ? "已保存" : "未设置"}</div>
        <div class="row">
          <input
            type="password"
            placeholder="粘贴密钥…"
            .value=${this.cfKeyDraft}
            @input=${(e: Event) => (this.cfKeyDraft = (e.target as HTMLInputElement).value)}
          />
          <button
            class="tonal"
            type="button"
            @click=${async () => {
              const r = await setCurseforgeKey(this.cfKeyDraft.trim() || null);
              showToast(r.message);
              this.cf = await curseforgeConfigured();
            }}
          >
            保存
          </button>
          <button
            class="ghost"
            type="button"
            @click=${async () => {
              await setCurseforgeKey(null);
              this.cf = await curseforgeConfigured();
              showToast("已清除");
            }}
          >
            清除
          </button>
        </div>
        <h2 style="margin-top:16px">云端助手（可选）</h2>
        <p>
          默认只用本机规则诊断。若要启用云端对话，在本机创建配置文件（不要把密钥写进项目或 git）：
        </p>
        <p><code>~/.config/easymc/llm.env</code></p>
        <p class="muted">
          写入：EASYMC_LLM_BASE_URL、EASYMC_LLM_API_KEY，可选 EASYMC_LLM_MODEL。也可用同名环境变量。
        </p>
        <div class="muted">
          状态：${
            this.llm?.configured
              ? `已连接 · ${this.llm.model ?? "模型未知"} · ${this.llm.base_url_host ?? ""}`
              : "未配置（仅本地助手）"
          }
        </div>
        <div class="row" style="margin-top:8px">
          <button
            class="ghost"
            type="button"
            @click=${async () => {
              this.llm = await agentLlmStatus();
              showToast(this.llm.configured ? "云端助手已就绪" : "仍是本地助手");
            }}
          >
            刷新状态
          </button>
        </div>
      </details>
    `;
  }

  private renderMp() {
    return html`<div class="card">
      <h2>联机</h2>
      <p>和朋友一起玩的相关选项会在联机功能上线后出现在这里。现在可以先去「联机」页看看预告。</p>
      <button
        class="ghost"
        type="button"
        @click=${() =>
          this.dispatchEvent(
            new CustomEvent("nav-change", { detail: "multiplayer", bubbles: true, composed: true }),
          )}
      >
        查看联机页
      </button>
    </div>`;
  }

  private renderPersonal() {
    return html`<div class="card">
        <h2>使用模式</h2>
        <p>Agent 模式让助手更显眼；标准模式是经典启动器布局。</p>
        <div class="row">
          <button
            class=${this.mode === "agent" ? "primary" : "ghost"}
            type="button"
            @click=${() => this.setMode("agent")}
          >
            Agent 模式
          </button>
          <button
            class=${this.mode === "standard" ? "primary" : "ghost"}
            type="button"
            @click=${() => this.setMode("standard")}
          >
            标准模式
          </button>
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
        <button
          class="ghost"
          type="button"
          @click=${async () => {
            await updateSettings({ onboarding_complete: false });
            showToast("下次打开将显示欢迎引导");
            this.dispatchEvent(new CustomEvent("onboarding-done", { bubbles: true, composed: true }));
            location.reload();
          }}
        >
          重新显示欢迎引导
        </button>
      </div>
      <div class="card">
        <h2>关于</h2>
        <p>
          EasyMC ${this.settings ? "0.1.0" : ""} · 开源 Minecraft Java 启动器。更多信息见「更多 → 关于」。
        </p>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    "easymc-settings-page": SettingsPage;
  }
}
