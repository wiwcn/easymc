import { LitElement, css, html, nothing } from "lit";
import { controlStyles, pageChromeStyles } from "../styles/controls";
import { customElement, state } from "lit/decorators.js";
import {
  agentDataDir,
  agentLlmConfigClear,
  agentLlmConfigGet,
  agentLlmConfigSave,
  agentLlmStatus,
  curseforgeConfigured,
  detectJava,
  ensureJavaInstall,
  getSettings,
  listenLogs,
  msaStatus,
  setCurseforgeKey,
  updateSettings,
  type JavaInstallDto,
} from "../api";
import type { AppMode, CurseForgeStatusDto, LlmConfigViewDto, LlmStatusDto, MsaStatusDto, SettingsDto } from "../types";
import { showToast } from "../toast";
import { syncIndicator } from "../lib/indicator";

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
  @state() private llmCfg: LlmConfigViewDto | null = null;
  @state() private llmBase = "";
  @state() private llmKey = "";
  @state() private llmModel = "";
  @state() private llmBusy = false;
  @state() private section: "launch" | "multiplayer" | "personal" | "other" = "launch";
  @state() private javaList: JavaInstallDto[] = [];
  @state() private javaBusy = false;
  @state() private javaStatus = "";
  @state() private javaLog = "";
  private unlistenJavaLog: (() => void) | null = null;

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
      :host {
        display: block;
        /* Column width comes from the shell (.page > * in app-shell.ts). */
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
      /* .tabs → controlStyles .md-tabs (MC-2, shared vocabulary + sliding indicator) */
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
        border-radius: var(--md-sys-shape-corner-full);
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
      .java-ok {
        color: var(--md-sys-color-primary);
        font-weight: 600;
        font-size: var(--easymc-type-label);
      }
      .java-warn {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-label);
      }
      .java-log {
        margin-top: 8px;
        padding: 10px 12px;
        border-radius: var(--md-sys-shape-corner-medium);
        background: var(--md-sys-color-surface-container-lowest);
        font-size: var(--easymc-type-meta);
        color: var(--md-sys-color-on-surface-variant);
        white-space: pre-wrap;
        word-break: break-all;
        max-height: 140px;
        overflow: auto;
      }
      code.path {
        font-size: var(--easymc-type-meta);
        word-break: break-all;
      }
    `,
  ];

  connectedCallback() {
    super.connectedCallback();
    const sec = sessionStorage.getItem("easymc-settings-section") as
      | "launch"
      | "multiplayer"
      | "personal"
      | "other"
      | null;
    if (sec && ["launch", "multiplayer", "personal", "other"].includes(sec)) {
      this.section = sec;
      sessionStorage.removeItem("easymc-settings-section");
    }
    void this.refresh();
  }

  disconnectedCallback() {
    this.unlistenJavaLog?.();
    this.unlistenJavaLog = null;
    super.disconnectedCallback();
  }

  protected updated() {
    syncIndicator(this.renderRoot);
  }

  private async refresh() {
    const [s, cf, msa, dir, llm, llmCfg, javas] = await Promise.all([
      getSettings(),
      curseforgeConfigured(),
      msaStatus(),
      agentDataDir(),
      agentLlmStatus(),
      agentLlmConfigGet(),
      detectJava(),
    ]);
    this.settings = s;
    this.mem = Math.min(MEM_MAX, Math.max(MEM_MIN, s.memory_mb ?? 2048));
    this.cf = cf;
    this.msa = msa;
    this.dataDir = dir.path;
    this.llm = llm;
    this.llmCfg = llmCfg;
    this.llmBase = llmCfg.base_url || "";
    this.llmModel = llmCfg.model || "deepseek-chat";
    this.llmKey = "";
    this.javaList = javas;
    if (!this.javaBusy) {
      const best = javas.find((j) => j.major >= 21) ?? javas[0];
      this.javaStatus = best
        ? `已就绪：Java ${best.major}（${best.source}）`
        : "尚未检测到可用的 Java。点下方按钮即可自动下载并配置。";
    }
  }

  private async setupJava() {
    if (this.javaBusy) return;
    this.javaBusy = true;
    this.javaLog = "";
    this.javaStatus = "正在准备 Java 21…";
    this.unlistenJavaLog?.();
    this.unlistenJavaLog = await listenLogs((line) => {
      this.javaLog = this.javaLog ? `${this.javaLog}\n${line}` : line;
      this.javaStatus = line;
    });
    try {
      const j = await ensureJavaInstall(21);
      this.javaStatus = `配置完成：Java ${j.major}`;
      this.javaLog = [
        this.javaLog,
        `路径：${j.path}`,
        `版本：${j.version_string}`,
        `来源：${j.source}`,
        "下次启动游戏会自动使用它。",
      ]
        .filter(Boolean)
        .join("\n");
      showToast(`Java ${j.major} 已就绪`);
      this.javaList = await detectJava();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.javaStatus = "配置失败";
      this.javaLog = this.javaLog ? `${this.javaLog}\n${msg}` : msg;
      showToast(`配置失败：${msg}`);
    } finally {
      this.unlistenJavaLog?.();
      this.unlistenJavaLog = null;
      this.javaBusy = false;
    }
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
      <p class="sub">启动环境、内存与使用偏好。账户请到「账户」页管理。高级选项默认折叠。</p>
      <div class="md-tabs" role="tablist" aria-label="设置分组">
        <span class="md-tab-ind" aria-hidden="true"></span>
        ${tabs.map(
          (t) => html`<button
            class=${this.section === t.id ? "on" : ""}
            type="button"
            role="tab"
            aria-selected=${this.section === t.id}
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
    const best = this.javaList.find((j) => j.major >= 21) ?? this.javaList[0] ?? null;
    const ready = !!best && best.major >= 21;
    return html`
      <div class="card">
        <h2>Java 运行环境</h2>
        <p>
          开玩 Minecraft 需要 Java。首次启动的「准备开玩环境」也会配置它；这里可随时再检查或重新下载。本机没有也没关系——点一次即可自动下载合适版本并交给 EasyMC 使用。
        </p>
        <div class=${ready ? "java-ok" : "java-warn"}>${this.javaStatus || (ready && best ? `已就绪：Java ${best.major}` : "尚未检测到可用的 Java")}</div>
        ${best
          ? html`<p class="muted" style="margin-top:6px">
              <code class="path">${best.path}</code>
            </p>`
          : nothing}
        <div class="row" style="margin-top:12px">
          <button
            class="primary"
            type="button"
            ?disabled=${this.javaBusy}
            @click=${() => void this.setupJava()}
          >
            ${this.javaBusy ? "正在配置…" : ready ? "重新检查 / 更新 Java" : "一键配置 Java"}
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.javaBusy}
            @click=${() => void this.refresh()}
          >
            刷新检测
          </button>
        </div>
        ${this.javaLog ? html`<div class="java-log" aria-live="polite">${this.javaLog}</div>` : nothing}
      </div>
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
      <div class="card">
        <h2>下载源</h2>
        <p>游戏文件与版本列表的下载地址。网络较慢时可改用 BMCLAPI 加速；失败时会自动回退官方源。</p>
        <div class="row">
          <select
            aria-label="下载源"
            .value=${this.settings?.download_source === "bmclapi" ? "bmclapi" : "official"}
            @change=${async (e: Event) => {
              const v = (e.target as HTMLSelectElement).value;
              this.settings = await updateSettings({ download_source: v });
              showToast(v === "bmclapi" ? "已切换为 BMCLAPI 加速" : "已切换为官方源");
            }}
          >
            <option value="official">官方</option>
            <option value="bmclapi">BMCLAPI 加速</option>
          </select>
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
        <h2 style="margin-top:16px">云端助手 API</h2>
        <p>
          默认只用本机规则诊断。填写下方 OpenAI 兼容接口后，助手页可走云端对话。密钥只保存在本机
          <code>~/.config/easymc/llm.env</code>，不会进 git。
        </p>
        <div class="muted" style="margin-bottom:8px">
          状态：${
            this.llm?.configured
              ? `已连接 · ${this.llm.model ?? "模型未知"} · ${this.llm.base_url_host ?? ""}`
              : "未配置（仅本地助手）"
          }
          ${this.llmCfg?.path ? html`<br /><code class="path">${this.llmCfg.path}</code>` : nothing}
        </div>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">Base URL</span>
          <input
            style="flex:1"
            placeholder="https://api.example.com 或 …/v1"
            .value=${this.llmBase}
            @input=${(e: Event) => (this.llmBase = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">API Key</span>
          <input
            style="flex:1"
            type="password"
            autocomplete="off"
            placeholder=${this.llmCfg?.api_key_set ? `已保存 ${this.llmCfg.api_key_hint}（留空则保持）` : "sk-…"}
            .value=${this.llmKey}
            @input=${(e: Event) => (this.llmKey = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">Model</span>
          <input
            style="flex:1"
            placeholder="deepseek-chat"
            .value=${this.llmModel}
            @input=${(e: Event) => (this.llmModel = (e.target as HTMLInputElement).value)}
          />
        </label>
        <div class="row" style="margin-top:8px">
          <button
            class="primary"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.saveLlm()}
          >
            保存并启用
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.clearLlm()}
          >
            清除
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.llmBusy}
            @click=${() => void this.refresh()}
          >
            刷新
          </button>
        </div>
      </details>
    `;
  }

  private async saveLlm() {
    this.llmBusy = true;
    try {
      this.llm = await agentLlmConfigSave(this.llmBase, this.llmKey, this.llmModel);
      this.llmCfg = await agentLlmConfigGet();
      this.llmKey = "";
      showToast(this.llm.configured ? "云端助手已启用" : "已保存，但仍未就绪");
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.llmBusy = false;
    }
  }

  private async clearLlm() {
    this.llmBusy = true;
    try {
      this.llm = await agentLlmConfigClear();
      this.llmCfg = await agentLlmConfigGet();
      this.llmBase = "";
      this.llmKey = "";
      this.llmModel = "deepseek-chat";
      showToast("已清除云端助手配置");
    } catch (e) {
      showToast(e instanceof Error ? e.message : String(e));
    } finally {
      this.llmBusy = false;
    }
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
        <div class="md-segmented" role="radiogroup" aria-label="使用模式">
          <span class="md-tab-ind" aria-hidden="true"></span>
          <button
            class=${this.mode === "agent" ? "on" : ""}
            type="button"
            role="radio"
            aria-checked=${this.mode === "agent"}
            @click=${() => this.setMode("agent")}
          >
            Agent 模式
          </button>
          <button
            class=${this.mode === "standard" ? "on" : ""}
            type="button"
            role="radio"
            aria-checked=${this.mode === "standard"}
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
