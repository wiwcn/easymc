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
  detectMinecraftDirs,
  ensureJavaInstall,
  getSettings,
  listenLogs,
  msaStatus,
  setCurseforgeKey,
  updateSettings,
  containerPreheat,
  containerPreheatStatus,
  containerProbe,
  containerRepair,
  containerWriteRegistries,
  probeDownloadMirrors,
  probeRegistryMirrors,
  type JavaInstallDto,
  type MinecraftDirDto,
} from "../api";
import type {
  AppMode,
  ContainerPreheatStatusDto,
  CurseForgeStatusDto,
  DownloadMirrorProbeDto,
  LlmConfigViewDto,
  LlmStatusDto,
  MsaStatusDto,
  RegistryMirrorProbeDto,
  SettingsDto,
} from "../types";
import { showToast } from "../toast";
import { friendlyError } from "../lib/friendly-error";
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
  @state() private mcDirs: MinecraftDirDto[] = [];
  @state() private javaBusy = false;
  @state() private preheatStatus: ContainerPreheatStatusDto | null = null;
  @state() private containerBusy = false;
  @state() private downloadProbes: DownloadMirrorProbeDto[] = [];
  @state() private downloadProbeBusy = false;
  @state() private registryProbes: RegistryMirrorProbeDto[] = [];
  @state() private registryProbeBusy = false;
  @state() private javaStatus = "";
  @state() private javaLog = "";
  private unlistenJavaLog: (() => void) | null = null;

  static styles = [
    controlStyles,
    pageChromeStyles,
    css`
    .probe-spin {
      display: inline-block; width: 12px; height: 12px; border-radius: 50%;
      border: 2px solid color-mix(in oklch, currentColor 35%, transparent);
      border-top-color: currentColor;
      animation: easymc-probe-spin 0.7s linear infinite;
      vertical-align: -1px; margin-right: 6px;
    }
    @keyframes easymc-probe-spin { to { transform: rotate(360deg); } }
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
        font-weight: var(--easymc-weight-medium);
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
        font-weight: var(--easymc-weight-medium);
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
      .mc-dirs {
        list-style: none;
        margin: 8px 0 0;
        padding: 0;
        display: grid;
        gap: 8px;
      }
      .mc-dirs li {
        display: flex;
        flex-direction: column;
        gap: 4px;
        padding: 10px 12px;
        border-radius: var(--md-sys-shape-corner-medium);
        background: var(--md-sys-color-surface-container-lowest);
      }
      .mc-tag {
        color: var(--md-sys-color-on-surface-variant);
        font-size: var(--easymc-type-meta);
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
    const [s, cf, msa, dir, llm, llmCfg, javas, mcDirs] = await Promise.all([
      getSettings(),
      curseforgeConfigured(),
      msaStatus(),
      agentDataDir(),
      agentLlmStatus(),
      agentLlmConfigGet(),
      detectJava(),
      detectMinecraftDirs(),
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
    this.mcDirs = mcDirs;
    void this.refreshPreheatStatus();
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
      const msg = friendlyError(e);
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

  private async refreshPreheatStatus() {
    try {
      this.preheatStatus = await containerPreheatStatus();
    } catch {
      this.preheatStatus = null;
    }
  }

  private async probeDownloads() {
    if (this.downloadProbeBusy) return;
    this.downloadProbeBusy = true;
    try {
      const list = await probeDownloadMirrors();
      this.downloadProbes = list;
      const best = list.find((r) => r.recommended) ?? list.find((r) => r.ok);
      if (best) {
        this.settings = await updateSettings({ download_source: best.download_source });
        showToast(`已选用最快下载源：${best.label_zh}${best.latency_ms != null ? `（${best.latency_ms} ms）` : ""}`);
      } else {
        showToast("测速完成，但没有可用的下载源");
      }
    } catch (e) {
      showToast(friendlyError(e, "下载源测速失败，请稍后重试"));
    } finally {
      this.downloadProbeBusy = false;
    }
  }

  private async probeRegistries() {
    if (this.registryProbeBusy) return;
    this.registryProbeBusy = true;
    try {
      const list = await probeRegistryMirrors();
      this.registryProbes = list;
      const best = list.find((r) => r.recommended) ?? list.find((r) => r.ok);
      if (best) {
        this.settings = await updateSettings({ container_mirror_profile: best.id });
        showToast(`已选用最快容器源：${best.label_zh}${best.latency_ms != null ? `（${best.latency_ms} ms）` : ""}`);
        await this.refreshPreheatStatus();
      } else {
        showToast("测速完成，但没有可用的容器镜像源");
      }
    } catch (e) {
      showToast(friendlyError(e, "加速源测速失败，请稍后重试"));
    } finally {
      this.registryProbeBusy = false;
    }
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
        <h2>已有 Minecraft 文件夹</h2>
        <p>
          自动扫描本机已存在的 <code class="path">.minecraft</code> 目录（官方启动器、Flatpak 等），方便沿用里面的版本、存档与设置。
        </p>
        ${this.mcDirs.length
          ? html`<ul class="mc-dirs">
              ${this.mcDirs.map(
                (d) => html`<li>
                  <code class="path">${d.path}</code>
                  <span class="mc-tag">${d.version_count} 个版本 · ${d.source}</span>
                </li>`,
              )}
            </ul>`
          : html`<div class="java-warn">未检测到已有的 Minecraft 文件夹。</div>`}
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
            aria-label="游戏内存"
            aria-valuetext="${(this.mem / 1024).toFixed(1)} GB（${this.mem} MB）"
            @input=${(e: Event) => {
              this.mem = Number((e.target as HTMLInputElement).value);
            }}
            @change=${() => void this.saveMem(this.mem)}
          />
          <span class="slider-value" aria-live="polite">${(this.mem / 1024).toFixed(this.mem % 1024 === 0 ? 0 : 1)}<span class="unit">GB</span>
            <span class="unit" style="opacity:.7;margin-left:4px">${this.mem} MB</span></span>
        </div>
        <div class="mem-hints">
          <span>${MEM_MIN / 1024} GB</span>
          <span>推荐 2–8 GB</span>
          <span>${MEM_MAX / 1024} GB</span>
        </div>
      </div>
      <div class="card">
        <h2>下载源</h2>
        <p>游戏文件与版本列表的下载地址。可测速后选用最快源（国内多为 BMCLAPI）；失败时会自动回退官方源。</p>
        <div class="row" style="margin-bottom:10px;align-items:center;gap:12px;flex-wrap:wrap">
          <select
            aria-label="下载源"
            .value=${this.settings?.download_source === "bmclapi" ? "bmclapi" : "official"}
            @change=${async (e: Event) => {
              const v = (e.target as HTMLSelectElement).value;
              this.settings = await updateSettings({ download_source: v });
              showToast(v === "bmclapi" ? "已切换为 BMCLAPI 加速" : "已切换为官方源");
            }}
          >
            <option value="official">官方源</option>
            <option value="bmclapi">BMCLAPI 加速</option>
          </select>
          <button
            class="btn ghost"
            ?disabled=${this.downloadProbeBusy}
            @click=${() => void this.probeDownloads()}
          >${this.downloadProbeBusy
            ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
            : "测速并选用最快"}</button>
        </div>
        ${this.downloadProbes.length
          ? html`<ul class="muted" style="margin:0;padding-left:18px;line-height:1.6">
              ${this.downloadProbes.map(
                (r) => html`<li>
                  ${r.label_zh}（${r.region_zh}）—
                  ${r.ok && r.latency_ms != null ? `${r.latency_ms} ms` : r.error_zh || "失败"}
                  ${r.recommended ? " · 推荐" : ""}
                </li>`,
              )}
            </ul>`
          : nothing}
      </div>
      <div class="card">
        <h2>云端助手接口</h2>
        <p>
          想让「助手」页真的会聊天、查崩溃，先在这里填好。密钥只保存在本机，不会上传。标准模式也能配，随时切助手模式即可用。
        </p>
        <div class="muted" style="margin-bottom:8px">
          状态：${
            this.llm?.configured
              ? `已连接 · ${this.llm.model ?? "模型未知"} · ${this.llm.base_url_host ?? ""}`
              : "还没配置（只能用本地规则提示）"
          }
        </div>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">接口地址</span>
          <input
            style="flex:1"
            placeholder="例如 https://api.deepseek.com/v1"
            .value=${this.llmBase}
            @input=${(e: Event) => (this.llmBase = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">密钥</span>
          <input
            style="flex:1"
            type="password"
            autocomplete="off"
            placeholder=${this.llmCfg?.api_key_set ? `已保存 ${this.llmCfg.api_key_hint}（留空则保持）` : "粘贴服务商给你的密钥"}
            .value=${this.llmKey}
            @input=${(e: Event) => (this.llmKey = (e.target as HTMLInputElement).value)}
          />
        </label>
        <label class="row" style="width:100%; margin-bottom:8px">
          <span style="min-width:5.5em">模型名称</span>
          <input
            style="flex:1"
            placeholder="例如 deepseek-chat"
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
            placeholder="微软登录用的应用 ID（一般不用填）"
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
        <h2 style="margin-top:16px">CurseForge 下载密钥（可选）</h2>
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
      </details>
      <details class="card" ?open=${!!this.settings?.container_mode_enabled}>
        <summary>容器隔离（可选 · 高级）</summary>
        <p>
          把每个版本的模组、配置分开存放，互不打架。更适合「助手模式」玩家；标准模式一般不用开。不装也完全能玩。
        </p>
        <p class="muted" style="margin:0 0 12px">
          ${this.preheatStatus
            ? (this.preheatStatus.warm ? "状态：已预热 · " : "状态：偏冷 · ") + this.preheatStatus.message_zh
            : "状态：尚未检测"}
        </p>
        <div class="row" style="margin-bottom:10px">
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
            <input
              type="checkbox"
              .checked=${!!this.settings?.container_mode_enabled}
              @change=${async (e: Event) => {
                const on = (e.target as HTMLInputElement).checked;
                this.settings = await updateSettings({ container_mode_enabled: on });
                showToast(on ? "已开启容器隔离" : "已关闭容器隔离");
              }}
            />
            开启容器隔离
          </label>
        </div>
        <div class="row" style="margin-bottom:10px;align-items:center;gap:12px;flex-wrap:wrap">
          <label for="ctr-mirror">下载加速源</label>
          <select
            id="ctr-mirror"
            aria-label="容器下载加速源"
            .value=${this.settings?.container_mirror_profile
              || (this.settings?.container_china_mirrors === false ? "official" : "daocloud")}
            @change=${async (e: Event) => {
              const v = (e.target as HTMLSelectElement).value;
              this.settings = await updateSettings({ container_mirror_profile: v });
              const labels: Record<string, string> = {
                daocloud: "国内线路 A",
                dockerproxy: "国内线路 B",
                onems: "国内线路 C",
                official: "官方线路（海外）",
              };
              showToast(`已切换为${labels[v] || v}`);
              await this.refreshPreheatStatus();
            }}
          >
            <option value="daocloud">国内线路 A</option>
            <option value="dockerproxy">国内线路 B</option>
            <option value="onems">国内线路 C</option>
            <option value="official">官方线路（海外）</option>
          </select>
          <button
            class="btn ghost"
            ?disabled=${this.registryProbeBusy}
            @click=${() => void this.probeRegistries()}
          >${this.registryProbeBusy
            ? html`<span class="probe-spin" aria-hidden="true"></span>测速中…`
            : "测速并选用最快"}</button>
        </div>
        ${this.registryProbes.length
          ? html`<ul class="muted" style="margin:0 0 10px;padding-left:18px;line-height:1.6">
              ${this.registryProbes.map(
                (r) => html`<li>
                  ${r.label_zh}（${r.region_zh}）—
                  ${r.ok && r.latency_ms != null ? `${r.latency_ms} ms` : r.error_zh || "失败"}
                  ${r.recommended ? " · 推荐" : ""}
                </li>`,
              )}
            </ul>`
          : nothing}
        <div class="row" style="margin-bottom:10px">
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
            <input
              type="checkbox"
              .checked=${this.settings?.container_saves_require_propose !== false}
              @change=${async (e: Event) => {
                const on = (e.target as HTMLInputElement).checked;
                this.settings = await updateSettings({ container_saves_require_propose: on });
                showToast(on ? "存档修改需确认" : "存档可直接写入（不推荐）");
              }}
            />
            改存档前先问我一声（推荐）
          </label>
        </div>
        <div class="row" style="margin-bottom:10px">
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer">
            <input
              type="checkbox"
              .checked=${this.settings?.container_auto_preheat !== false}
              @change=${async (e: Event) => {
                const on = (e.target as HTMLInputElement).checked;
                this.settings = await updateSettings({ container_auto_preheat: on });
                showToast(on ? "已开启后台预热" : "已关闭后台预热");
              }}
            />
            后台预热，缩短下次启动等待
          </label>
        </div>
        <div class="row" style="flex-wrap:wrap;gap:8px">
          <button
            class="ghost"
            type="button"
            ?disabled=${this.containerBusy}
            @click=${async () => {
              try {
                const p = await containerProbe();
                showToast(p.message_zh);
                await this.refreshPreheatStatus();
              } catch (e) {
                showToast(friendlyError(e));
              }
            }}
          >
            检查隔离环境
          </button>
          <button
            class="tonal"
            type="button"
            ?disabled=${this.containerBusy}
            @click=${async () => {
              this.containerBusy = true;
              try {
                const r = await containerPreheat(true);
                showToast(r.messages_zh.join("；") || (r.ok ? "预热完成" : "预热未完成"));
                await this.refreshPreheatStatus();
              } catch (e) {
                showToast(friendlyError(e));
              } finally {
                this.containerBusy = false;
              }
            }}
          >
            立刻预热
          </button>
          <button
            class="ghost"
            type="button"
            ?disabled=${this.containerBusy}
            @click=${async () => {
              try {
                const r = await containerWriteRegistries();
                showToast(r.message_zh);
              } catch (e) {
                showToast(friendlyError(e));
              }
            }}
          >
            应用加速源配置
          </button>
          <button
            class=${this.settings?.container_mode_enabled ? "tonal" : "ghost"}
            type="button"
            ?disabled=${this.containerBusy || !this.settings?.container_mode_enabled}
            title=${this.settings?.container_mode_enabled ? "修复隔离环境" : "先开启容器隔离后再修复"}
            @click=${async () => {
              this.containerBusy = true;
              try {
                const r = await containerRepair({ reprovision: true });
                showToast(r.messages_zh.slice(-2).join("；") || (r.ok ? "修复完成" : "修复未完成"));
                await this.refreshPreheatStatus();
              } catch (e) {
                showToast(friendlyError(e));
              } finally {
                this.containerBusy = false;
              }
            }}
          >
            修复隔离环境
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
      showToast(friendlyError(e));
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
      showToast(friendlyError(e));
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
        <p>助手模式：侧栏有「助手」，适合不想折腾的玩家；标准模式：经典启动器布局，需要时仍可打开助手。容器隔离只在需要时再开。</p>
        <div class="md-segmented" role="radiogroup" aria-label="使用模式">
          <span class="md-tab-ind" aria-hidden="true"></span>
          <button
            class=${this.mode === "agent" ? "on" : ""}
            type="button"
            role="radio"
            aria-checked=${this.mode === "agent"}
            @click=${() => this.setMode("agent")}
          >
            助手模式
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
    showToast(m === "agent" ? "已切换到助手模式（容器隔离可在「启动」里选开）" : "已切换到标准模式");
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
          EasyMC ${this.settings ? "0.1.2" : ""} · 开源 Minecraft Java 启动器。更多信息见「更多 → 关于」。
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
