import type { AccountDto, AccountsState, AgentAskResponse, AppMode, ChangeProposalDto, ContentInstallResultDto, CurseForgeStatusDto, DiagnoseReportDto, ExportResultDto, ImportResultDto, InstanceDto, InstancesState, LoaderVersionItem, InstalledModEntryDto, LocalEntryDto, LlmConfigViewDto, LlmStatusDto, ModInstallResultDto, ModpackInstallResultDto, ModProjectDto, ModVersionDto, MsaStatusDto, PreferredLogin, ProjectTypeId, SettingsDto, VersionSummary } from "./types";
import { versionId } from "./types";
import type { InstallProgressDto } from "./install-progress";
export type { InstallProgressDto } from "./install-progress";

async function invokeSafe<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<T>(cmd, args);
  } catch (e) {
    console.warn("invoke failed", cmd, e);
    return null;
  }
}

async function invokeOrThrow<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

function mapInstance(i: InstanceDto): InstanceDto {
  return {
    ...i,
    version: versionId(i.version),
    loader: String(i.loader).toLowerCase(),
  };
}

function mapState(raw: InstancesState | InstanceDto[] | null): InstancesState {
  if (!raw) {
    return {
      instances: [
        {
          id: "preview-1211-fabric",
          name: "生存世界",
          version: "1.21.1",
          loader: "fabric",
          loader_version: "0.16.9",
          description: "日常生存，装好模组就能开玩。",
          version_ready: true,
          category: "生存",
          last_played: "2026-10-24T08:32:00+08:00",
        },
        {
          id: "preview-1211-vanilla",
          name: "创造建造",
          version: "1.21.1",
          loader: "vanilla",
          description: "纯净原版建造。",
          version_ready: true,
          last_played: "2026-10-23T16:10:00+08:00",
        },
        {
          id: "preview-1201-forge",
          name: "模组冒险",
          version: "1.20.1",
          loader: "forge",
          loader_version: "47.3.0",
          description: "Forge 模组包预览。",
          version_ready: false,
          category: "模组",
        },
        {
          id: "preview-121-test",
          name: "测试世界",
          version: "1.21",
          loader: "fabric",
          description: "试玩快照功能。",
          version_ready: true,
        },
        {
          id: "preview-1192-quilt",
          name: "旧版怀旧",
          version: "1.19.2",
          loader: "quilt",
          loader_version: "0.20.0",
          description: "怀旧生存。",
          version_ready: true,
        },
      ],
      active_id: "preview-1211-fabric",
    };
  }
  if (Array.isArray(raw)) {
    return {
      instances: raw.map(mapInstance),
      active_id: raw[0]?.id ?? null,
    };
  }
  return {
    instances: (raw.instances ?? []).map(mapInstance),
    active_id: raw.active_id ?? null,
  };
}

export async function listInstancesState(): Promise<InstancesState> {
  const raw = await invokeSafe<InstancesState | InstanceDto[]>("list_instances");
  return mapState(raw);
}

export async function listInstances(): Promise<InstanceDto[]> {
  return (await listInstancesState()).instances;
}

export async function createInstance(
  name: string,
  version: string,
  loader = "vanilla",
  loaderVersion?: string | null,
): Promise<InstanceDto> {
  const raw = await invokeOrThrow<InstanceDto>("create_instance", {
    name,
    version,
    loader,
    loaderVersion: loaderVersion || null,
  });
  return mapInstance(raw);
}

export async function setActiveInstance(instanceId: string): Promise<InstancesState> {
  const raw = await invokeSafe<InstancesState>("set_active_instance_cmd", { instanceId });
  if (raw) return mapState(raw);
  // browser preview: keep selection client-side
  const st = await listInstancesState();
  return { instances: st.instances, active_id: instanceId };
}


const PREVIEW_ACCOUNTS: AccountDto[] = [
  {
    id: "preview-ms",
    username: "SteveMC",
    kind: "microsoft",
    uuid: "00000000-0000-0000-0000-0000000000aa",
  },
  {
    id: "preview-offline",
    username: "EasyPlayer",
    kind: "offline",
    uuid: "00000000-0000-0000-0000-000000000001",
  },
  {
    id: "preview-skin",
    username: "LittleSkinUser",
    kind: "skin_station",
    uuid: "00000000-0000-0000-0000-0000000000bb",
  },
];

const PREVIEW_ACTIVE_KEY = "easymc.previewActiveAccount";
const PREVIEW_EXTRA_KEY = "easymc.previewExtraAccounts";

function previewAccountsState(): AccountsState {
  let extra: AccountDto[] = [];
  try {
    extra = JSON.parse(localStorage.getItem(PREVIEW_EXTRA_KEY) || "[]");
  } catch { /* ignore */ }
  const accounts = [...PREVIEW_ACCOUNTS, ...extra];
  let active_id = localStorage.getItem(PREVIEW_ACTIVE_KEY) || accounts[0]?.id || null;
  if (active_id && !accounts.some((a) => a.id === active_id)) active_id = accounts[0]?.id ?? null;
  return { accounts, active_id };
}

function savePreviewAccounts(st: AccountsState) {
  try {
    localStorage.setItem(PREVIEW_ACTIVE_KEY, st.active_id ?? "");
    const extra = st.accounts.filter((a) => !PREVIEW_ACCOUNTS.some((p) => p.id === a.id));
    localStorage.setItem(PREVIEW_EXTRA_KEY, JSON.stringify(extra));
  } catch { /* ignore */ }
}

export async function listAccountsState(): Promise<AccountsState> {
  const raw = await invokeSafe<AccountsState | AccountDto[]>("list_accounts_state");
  if (raw && !Array.isArray(raw) && Array.isArray(raw.accounts)) {
    return { accounts: raw.accounts, active_id: raw.active_id ?? raw.accounts[0]?.id ?? null };
  }
  if (Array.isArray(raw)) {
    return { accounts: raw, active_id: raw[0]?.id ?? null };
  }
  // fall through: try legacy list_accounts
  const legacy = await invokeSafe<AccountDto[]>("list_accounts");
  if (legacy && legacy.length) {
    return { accounts: legacy, active_id: legacy[0]?.id ?? null };
  }
  return previewAccountsState();
}

export async function listAccounts(): Promise<AccountDto[]> {
  return (await listAccountsState()).accounts;
}

export async function setActiveAccount(accountId: string): Promise<AccountsState> {
  const raw = await invokeSafe<AccountsState>("set_active_account", { accountId });
  if (raw) return raw;
  const st = previewAccountsState();
  if (!st.accounts.some((a) => a.id === accountId)) return st;
  st.active_id = accountId;
  savePreviewAccounts(st);
  return st;
}

export async function removeAccount(accountId: string): Promise<AccountsState> {
  const raw = await invokeSafe<AccountsState>("remove_account", { accountId });
  if (raw) return raw;
  const st = previewAccountsState();
  st.accounts = st.accounts.filter((a) => a.id !== accountId);
  if (st.active_id === accountId) st.active_id = st.accounts[0]?.id ?? null;
  savePreviewAccounts(st);
  return st;
}

export async function createOffline(username: string): Promise<AccountDto | null> {
  const acc = await invokeSafe<AccountDto>("create_offline", { username });
  if (acc) return acc;
  // browser preview
  const name = username.trim();
  if (!name || [...name].length < 2 || [...name].length > 16) return null;
  const id = `preview-offline-${Date.now()}`;
  const dto: AccountDto = {
    id,
    username: name,
    kind: "offline",
    uuid: `00000000-0000-0000-0000-${String(Date.now()).slice(-12).padStart(12, "0")}`,
  };
  const st = previewAccountsState();
  st.accounts = st.accounts.filter((a) => a.username !== name);
  st.accounts.push(dto);
  st.active_id = dto.id;
  savePreviewAccounts(st);
  return dto;
}

export async function listVersions(includeSnapshots = false): Promise<VersionSummary[]> {
  const remote = await invokeSafe<VersionSummary[]>("list_versions", { includeSnapshots });
  if (remote && remote.length) return remote;
  const preview: VersionSummary[] = [
    { id: "1.21.1", type: "release", url: "", installed: true },
    { id: "1.21", type: "release", url: "", installed: false },
    { id: "1.20.4", type: "release", url: "", installed: true },
    { id: "1.20.1", type: "release", url: "", installed: false },
    { id: "1.19.4", type: "release", url: "", installed: false },
    { id: "1.19.2", type: "release", url: "", installed: false },
    { id: "1.18.2", type: "release", url: "", installed: false },
    { id: "1.16.5", type: "release", url: "", installed: false },
    { id: "1.12.2", type: "release", url: "", installed: false },
    { id: "1.8.9", type: "release", url: "", installed: false },
    { id: "24w44a", type: "snapshot", url: "", installed: false },
    { id: "24w40a", type: "snapshot", url: "", installed: false },
  ];
  return includeSnapshots ? preview : preview.filter((v) => (v.type ?? v.version_type) !== "snapshot");
}

export async function installVersion(version: string, withAssets = true): Promise<string> {
  return invokeOrThrow<string>("install_mc_version", { version, withAssets });
}

export async function listLoaderVersions(
  loader: string,
  gameVersion: string,
): Promise<LoaderVersionItem[]> {
  const remote = await invokeSafe<LoaderVersionItem[]>("list_loader_versions", { loader, gameVersion });
  if (remote && remote.length) return remote;
  void gameVersion;
  if (loader === "vanilla") return [];
  const presets: Record<string, string> = {
    fabric: "0.16.9",
    quilt: "0.26.1",
    forge: "47.3.0",
    neoforge: "21.1.0",
  };
  const base = presets[loader] ?? "1.0.0";
  const parts = base.split(".");
  const last = Number(parts[parts.length - 1] || 0);
  const prev = [...parts.slice(0, -1), String(Math.max(0, last - 1))].join(".");
  return [
    { version: base, stable: true, intermediary: "" },
    { version: prev, stable: true, intermediary: "" },
    { version: `${base}-beta`, stable: false, intermediary: "" },
  ];
}

export async function installLoaderVersion(
  gameVersion: string,
  loader: string,
  loaderVersion: string | null = null,
  withAssets = true,
): Promise<string> {
  return invokeOrThrow<string>("install_loader_version", {
    gameVersion,
    loader,
    loaderVersion,
    withAssets,
  });
}

export async function launchInstance(instanceId: string): Promise<string> {
  try {
    const res = await invokeOrThrow<{ ok: boolean; message: string }>("launch_instance", {
      instanceId,
    });
    return res.message;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(msg);
  }
}

export async function stopInstance(): Promise<void> {
  await invokeSafe("stop_instance");
}

export async function getLogs(): Promise<string[]> {
  return (await invokeSafe<string[]>("get_logs")) ?? [];
}

export type JavaInstallDto = {
  path: string;
  version_string: string;
  major: number;
  source: string;
};

export async function detectJava(): Promise<JavaInstallDto[]> {
  const list = await invokeSafe<JavaInstallDto[]>("detect_java_cmd");
  return list ?? [];
}

export type MinecraftDirDto = {
  path: string;
  source: string;
  version_count: number;
};

/** Existing `.minecraft` folders found on this machine (official launcher, Flatpak, …). */
export async function detectMinecraftDirs(): Promise<MinecraftDirDto[]> {
  const list = await invokeSafe<MinecraftDirDto[]>("detect_minecraft_dirs_cmd");
  return list ?? [];
}

/** Find a suitable Java or download Temurin into EasyMC runtimes. */
export async function ensureJavaInstall(major = 21): Promise<JavaInstallDto> {
  return invokeOrThrow<JavaInstallDto>("ensure_java_cmd", { major });
}

export async function ensureJava(major = 21): Promise<string> {
  const j = await ensureJavaInstall(major);
  return `${j.path} (Java ${j.major})`;
}

export async function listenLogs(handler: (line: string) => void): Promise<() => void> {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<string>("launch-log", (e) => handler(e.payload));
    return () => un();
  } catch {
    return () => undefined;
  }
}

export async function listenInstallProgress(
  handler: (p: InstallProgressDto) => void,
): Promise<() => void> {
  try {
    const { listen } = await import("@tauri-apps/api/event");
    const un = await listen<InstallProgressDto>("install-progress", (e) => handler(e.payload));
    return () => un();
  } catch {
    return () => undefined;
  }
}


export async function curseforgeConfigured(): Promise<CurseForgeStatusDto> {
  return (
    (await invokeSafe<CurseForgeStatusDto>("curseforge_configured")) ?? {
      configured: false,
      env: "EASYMC_CURSEFORGE_API_KEY",
      source: "none",
      console_url: "https://console.curseforge.com/",
      key_len: 0,
      hint: "未配置 CurseForge API Key（浏览器预览）",
    }
  );
}

export async function setCurseforgeKey(key: string | null): Promise<{
  ok: boolean;
  configured: boolean;
  source: string;
  key_len: number;
  message: string;
}> {
  return invokeOrThrow("set_curseforge_key_cmd", { key });
}

export async function searchMods(
  source: string,
  query: string,
  gameVersion?: string | null,
  loader?: string | null,
  limit = 20,
): Promise<ModProjectDto[]> {
  // Use invokeOrThrow so preview / network failures surface as errors
  // (UI can show 「模组列表暂时加载不了」 instead of a false empty state).
  return invokeOrThrow<ModProjectDto[]>("search_mods_cmd", {
    source,
    query: query || null,
    gameVersion: gameVersion || null,
    loader: loader || null,
    limit,
  });
}

export async function listModVersions(
  source: string,
  projectId: string,
  gameVersion?: string | null,
  loader?: string | null,
): Promise<ModVersionDto[]> {
  return (
    (await invokeSafe<ModVersionDto[]>("list_mod_versions_cmd", {
      source,
      projectId,
      gameVersion: gameVersion || null,
      loader: loader || null,
    })) ?? []
  );
}

export async function installMod(
  source: string,
  projectId: string,
  versionId?: string | null,
  instanceId?: string | null,
): Promise<ModInstallResultDto> {
  return invokeOrThrow<ModInstallResultDto>("install_mod_cmd", {
    source,
    projectId,
    versionId: versionId || null,
    instanceId: instanceId || null,
  });
}

export async function listInstalledMods(instanceId?: string | null): Promise<InstalledModEntryDto[]> {
  return (
    (await invokeSafe<InstalledModEntryDto[]>("list_installed_mods_cmd", {
      instanceId: instanceId || null,
    })) ?? []
  );
}

export async function removeMod(
  filename: string,
  instanceId?: string | null,
): Promise<string> {
  return invokeOrThrow<string>("remove_mod_cmd", {
    filename,
    instanceId: instanceId || null,
  });
}

export async function setModDisabled(
  filename: string,
  disabled: boolean,
  instanceId?: string | null,
): Promise<string> {
  return invokeOrThrow<string>("set_mod_disabled_cmd", {
    filename,
    disabled,
    instanceId: instanceId || null,
  });
}



export async function modDepGraph(instanceId?: string | null): Promise<import("./types").DepGraphReportDto> {
  return invokeOrThrow<import("./types").DepGraphReportDto>("mod_dep_graph_cmd", {
    instanceId: instanceId || null,
  });
}

export async function fetchMissingModDeps(
  instanceId?: string | null,
  onlyIds?: string[] | null,
): Promise<import("./types").FetchMissingResultDto> {
  return invokeOrThrow<import("./types").FetchMissingResultDto>("fetch_missing_mod_deps_cmd", {
    instanceId: instanceId || null,
    onlyIds: onlyIds ?? null,
  });
}

export async function agentLlmStatus(): Promise<LlmStatusDto> {
  return (
    (await invokeSafe<LlmStatusDto>("agent_llm_status")) ?? {
      configured: false,
      mode: "rules_only",
    }
  );
}

export async function agentLlmConfigGet(): Promise<LlmConfigViewDto> {
  return (
    (await invokeSafe<LlmConfigViewDto>("agent_llm_config_get")) ?? {
      path: null,
      base_url: "",
      model: "deepseek-chat",
      api_key_set: false,
      api_key_hint: "",
      configured: false,
    }
  );
}

export async function agentLlmConfigSave(
  baseUrl: string,
  apiKey: string,
  model: string,
): Promise<LlmStatusDto> {
  return invokeOrThrow<LlmStatusDto>("agent_llm_config_save", {
    baseUrl,
    apiKey,
    model,
  });
}

export async function agentLlmConfigClear(): Promise<LlmStatusDto> {
  return invokeOrThrow<LlmStatusDto>("agent_llm_config_clear");
}

export async function agentDiagnose(instanceId?: string | null): Promise<DiagnoseReportDto> {
  return invokeOrThrow<DiagnoseReportDto>("agent_diagnose", {
    instanceId: instanceId || null,
  });
}

export async function agentAsk(
  text: string,
  opts?: {
    instanceId?: string | null;
    rulesOnly?: boolean;
    history?: { role: string; content: string }[];
  },
): Promise<AgentAskResponse> {
  return invokeOrThrow<AgentAskResponse>("agent_ask", {
    text,
    instanceId: opts?.instanceId || null,
    rulesOnly: opts?.rulesOnly ?? false,
    history: opts?.history ?? [],
  });
}

export async function agentDataDir(): Promise<{ path: string; message: string }> {
  return (
    (await invokeSafe<{ path: string; message: string }>("agent_data_dir")) ?? {
      path: "",
      message: "（浏览器预览）",
    }
  );
}

export async function agentBumpMemory(mb: number): Promise<{ ok: boolean; message: string }> {
  return invokeOrThrow("agent_bump_memory", { mb });
}

export async function agentExportDiagnostics(
  instanceId?: string | null,
): Promise<{ ok: boolean; path?: string; message: string }> {
  return invokeOrThrow("agent_export_diagnostics", {
    instanceId: instanceId || null,
  });
}

export async function agentApplyProposal(
  proposal: ChangeProposalDto,
): Promise<{ ok: boolean; message: string; message_zh?: string }> {
  return invokeOrThrow("agent_apply_proposal", { proposal });
}

export async function agentUndoProposal(
  proposal: ChangeProposalDto,
): Promise<{ ok: boolean; message: string; message_zh?: string }> {
  return invokeOrThrow("agent_undo_proposal", { proposal });
}


export async function exportInstance(
  instanceId: string,
  pack = false,
  notes?: string | null,
): Promise<ExportResultDto> {
  return invokeOrThrow<ExportResultDto>("export_instance_cmd", {
    instanceId,
    pack,
    notes: notes || null,
  });
}

export async function importInstance(
  path: string,
  opts?: { name?: string | null; applyMemory?: boolean; offline?: boolean },
): Promise<ImportResultDto> {
  return invokeOrThrow<ImportResultDto>("import_instance_cmd", {
    path,
    name: opts?.name || null,
    applyMemory: opts?.applyMemory ?? false,
    offline: opts?.offline ?? false,
  });
}


function previewOnboardingForced(): boolean {
  try {
    if (typeof localStorage !== "undefined" && localStorage.getItem("easymc.previewOnboarding") === "1") {
      return true;
    }
    if (typeof location !== "undefined" && new URLSearchParams(location.search).get("onboard") === "1") {
      return true;
    }
  } catch {
    /* ignore */
  }
  return false;
}

const PREVIEW_SETTINGS_KEY = "easymc.previewSettings";

function defaultPreviewSettings(): SettingsDto {
  return {
    memory_mb: 2048,
    mode: "standard",
    // Browser preview skips onboarding unless ?onboard=1 or localStorage easymc.previewOnboarding=1
    onboarding_complete: !previewOnboardingForced(),
    download_source: "official",
    container_mode_enabled: false,
    container_china_mirrors: true,
    container_setup_done: false,
    container_saves_require_propose: true,
    container_auto_preheat: true,
  };
}

function readPreviewSettings(): SettingsDto {
  try {
    const raw = localStorage.getItem(PREVIEW_SETTINGS_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<SettingsDto>;
      const base = defaultPreviewSettings();
      const merged: SettingsDto = { ...base, ...parsed };
      // Force-onboard query/flag always wins until completeOnboarding clears it.
      if (previewOnboardingForced()) merged.onboarding_complete = false;
      return merged;
    }
  } catch {
    /* ignore */
  }
  return defaultPreviewSettings();
}

function writePreviewSettings(s: SettingsDto): void {
  try {
    localStorage.setItem(PREVIEW_SETTINGS_KEY, JSON.stringify(s));
  } catch {
    /* ignore */
  }
}

export async function getSettings(): Promise<SettingsDto> {
  const remote = await invokeSafe<SettingsDto>("get_settings");
  if (remote) return remote;
  return readPreviewSettings();
}

export async function updateSettings(patch: {
  mode?: AppMode | string;
  onboarding_complete?: boolean;
  preferred_login?: PreferredLogin | string | null;
  /** Absolute path to java; empty/null clears → auto-pick. */
  preferred_java?: string | null;
  memory_mb?: number;
  msa_client_id?: string | null;
  clear_msa_client_id?: boolean;
  download_source?: string;
  container_mode_enabled?: boolean;
  container_china_mirrors?: boolean;
  container_setup_done?: boolean;
  container_saves_require_propose?: boolean;
  container_auto_preheat?: boolean;
  container_mirror_profile?: string;
}): Promise<SettingsDto> {
  try {
    return await invokeOrThrow<SettingsDto>("update_settings_cmd", { patch });
  } catch {
    // Browser preview: persist mode / container flags so settings match onboarding.
    const cur = readPreviewSettings();
    const next: SettingsDto = {
      ...cur,
      ...(patch.mode != null ? { mode: (patch.mode as AppMode) || cur.mode } : {}),
      ...(patch.onboarding_complete != null ? { onboarding_complete: !!patch.onboarding_complete } : {}),
      ...(patch.memory_mb != null ? { memory_mb: patch.memory_mb } : {}),
      ...(patch.download_source != null ? { download_source: patch.download_source } : {}),
      ...(patch.container_mode_enabled != null ? { container_mode_enabled: !!patch.container_mode_enabled } : {}),
      ...(patch.container_china_mirrors != null ? { container_china_mirrors: !!patch.container_china_mirrors } : {}),
      ...(patch.container_setup_done != null ? { container_setup_done: !!patch.container_setup_done } : {}),
      ...(patch.container_saves_require_propose != null
        ? { container_saves_require_propose: !!patch.container_saves_require_propose }
        : {}),
      ...(patch.container_auto_preheat != null ? { container_auto_preheat: !!patch.container_auto_preheat } : {}),
      ...(patch.container_mirror_profile != null ? { container_mirror_profile: patch.container_mirror_profile } : {}),
      ...(patch.preferred_login !== undefined ? { preferred_login: patch.preferred_login as never } : {}),
      ...(patch.preferred_java !== undefined
        ? { preferred_java: (patch.preferred_java ?? "").trim() || null }
        : {}),
    };
    writePreviewSettings(next);
    return next;
  }
}

export async function containerProbe(): Promise<import("./types").ContainerRuntimeProbeDto> {
  return (
    (await invokeSafe<import("./types").ContainerRuntimeProbeDto>("container_probe_cmd")) ?? {
      status: "not_installed",
      message_zh: "当前是预览，隔离环境需在桌面版里配置；也可以先跳过。",
      platform: "other",
      podman: null,
    }
  );
}

export async function containerSetupPlan(): Promise<import("./types").ContainerSetupPlanDto> {
  return (
    (await invokeSafe<import("./types").ContainerSetupPlanDto>("container_setup_plan_cmd")) ?? {
      actions: [],
      probe: await containerProbe(),
      summary_zh: "容器一键配置（预览）",
    }
  );
}

export async function containerOneClickSetup(
  preferChinaMirrors?: boolean | null,
): Promise<import("./types").ContainerSetupResultDto> {
  return invokeOrThrow<import("./types").ContainerSetupResultDto>("container_one_click_setup_cmd", {
    preferChinaMirrors: preferChinaMirrors ?? null,
  });
}

export async function containerWriteRegistries(): Promise<import("./types").ChinaRegistriesResultDto> {
  return invokeOrThrow<import("./types").ChinaRegistriesResultDto>("container_write_registries_cmd");
}

export async function containerListVersions(): Promise<import("./types").ContainerVersionSummaryDto[]> {
  return (await invokeSafe<import("./types").ContainerVersionSummaryDto[]>("container_list_versions_cmd")) ?? [];
}

export async function containerCreateVersion(
  name: string,
  version: string,
  loader?: string | null,
  loaderVersion?: string | null,
): Promise<InstanceDto> {
  return invokeOrThrow<InstanceDto>("container_create_version_cmd", {
    name,
    version,
    loader: loader ?? null,
    loaderVersion: loaderVersion ?? null,
  });
}

export async function containerAttach(instanceId: string): Promise<InstanceDto> {
  return invokeOrThrow<InstanceDto>("container_attach_cmd", { instanceId });
}

export async function containerDeleteVersion(
  instanceId: string,
  deleteFiles = true,
): Promise<void> {
  await invokeOrThrow<void>("container_delete_version_cmd", {
    instanceId,
    deleteFiles,
  });
}

export async function completeOnboarding(
  mode: AppMode | string,
  preferredLogin?: PreferredLogin | string | null,
): Promise<SettingsDto> {
  try {
    return await invokeOrThrow<SettingsDto>("complete_onboarding_cmd", {
      mode,
      preferredLogin: preferredLogin ?? null,
    });
  } catch {
    // Browser preview: clear force flag so shell leaves onboarding, persist mode.
    try {
      localStorage.removeItem("easymc.previewOnboarding");
      if (typeof history !== "undefined" && typeof location !== "undefined" && location.search.includes("onboard=1")) {
        const u = new URL(location.href);
        u.searchParams.delete("onboard");
        history.replaceState(null, "", u.pathname + u.search + u.hash);
      }
    } catch {
      /* ignore */
    }
    const cur = readPreviewSettings();
    const next: SettingsDto = {
      ...cur,
      memory_mb: cur.memory_mb ?? 2048,
      mode: (mode as AppMode) || cur.mode || "standard",
      onboarding_complete: true,
      preferred_login: (preferredLogin as never) ?? cur.preferred_login,
      container_setup_done: cur.container_setup_done ?? true,
    };
    writePreviewSettings(next);
    return next;
  }
}

export async function msaStatus(): Promise<MsaStatusDto> {
  return (
    (await invokeSafe<MsaStatusDto>("msa_status_cmd")) ?? {
      configured: false,
      using_temporary_client_id: false,
      oauth_ready: false,
      message_zh: "正版登录即将可用",
    }
  );
}

export async function createOfflineOrThrow(username: string): Promise<AccountDto> {
  try {
    return await invokeOrThrow<AccountDto>("create_offline", { username });
  } catch {
    const acc = await createOffline(username);
    if (!acc) throw new Error("请输入 2–16 个字符的名字");
    return acc;
  }
}

export async function startMicrosoftLogin(): Promise<AccountDto> {
  return invokeOrThrow<AccountDto>("start_microsoft_login");
}

export async function msaDeviceCode(): Promise<import("./types").DeviceCodeDto> {
  return invokeOrThrow("msa_device_code_cmd");
}

export async function msaPoll(info: import("./types").DeviceCodeDto): Promise<AccountDto> {
  return invokeOrThrow<AccountDto>("msa_poll_cmd", { info });
}

export async function updateInstance(
  instanceId: string,
  patch: Record<string, unknown>,
): Promise<InstanceDto> {
  return mapInstance(await invokeOrThrow<InstanceDto>("update_instance_cmd", { instanceId, patch }));
}

export async function renameInstance(instanceId: string, name: string): Promise<InstanceDto> {
  return mapInstance(await invokeOrThrow<InstanceDto>("rename_instance_cmd", { instanceId, name }));
}

export async function duplicateInstance(instanceId: string, name?: string | null): Promise<InstanceDto> {
  return mapInstance(await invokeOrThrow<InstanceDto>("duplicate_instance_cmd", { instanceId, name: name || null }));
}

export async function deleteInstance(instanceId: string, deleteFiles = true): Promise<void> {
  await invokeOrThrow("delete_instance_cmd", { instanceId, deleteFiles });
}

export async function openInstanceFolder(instanceId: string, kind?: string | null): Promise<string> {
  return invokeOrThrow<string>("open_instance_folder_cmd", { instanceId, kind: kind || null });
}

export async function writeLaunchScript(instanceId: string): Promise<{ path: string; message_zh: string }> {
  return invokeOrThrow("write_launch_script_cmd", { instanceId });
}

export async function uploadLatestLog(
  instanceId?: string | null,
): Promise<import("./types").MclogsUploadResultDto> {
  return invokeOrThrow("upload_latest_log_cmd", {
    instanceId: instanceId || null,
  });
}

export async function listLocalContent(instanceId: string, kind: string): Promise<LocalEntryDto[]> {
  return (await invokeSafe<LocalEntryDto[]>("list_local_content_cmd", { instanceId, kind })) ?? [];
}

export async function deleteLocalContent(instanceId: string, kind: string, name: string): Promise<string> {
  return invokeOrThrow("delete_local_content_cmd", { instanceId, kind, name });
}

export async function searchContent(
  source: string,
  projectType: ProjectTypeId | string,
  query: string,
  gameVersion?: string | null,
  loader?: string | null,
  limit = 20,
): Promise<ModProjectDto[]> {
  return invokeOrThrow<ModProjectDto[]>("search_content_cmd", {
      source,
      projectType,
      query: query || null,
      gameVersion: gameVersion || null,
      loader: loader || null,
      limit,
    });
}

export async function installContent(
  source: string,
  projectType: ProjectTypeId | string,
  projectId: string,
  opts?: { versionId?: string | null; instanceId?: string | null; world?: string | null },
): Promise<ContentInstallResultDto> {
  return invokeOrThrow("install_content_cmd", {
    source,
    projectType,
    projectId,
    versionId: opts?.versionId || null,
    instanceId: opts?.instanceId || null,
    world: opts?.world || null,
  });
}

export async function installModpack(
  source: string,
  projectId: string,
  opts?: { versionId?: string | null; name?: string | null },
): Promise<ModpackInstallResultDto> {
  return invokeOrThrow("install_modpack_cmd", {
    source,
    projectId,
    versionId: opts?.versionId || null,
    name: opts?.name || null,
  });
}

export async function containerProvision(instanceId: string) {
  return invokeOrThrow<import("./types").ContainerProvisionDto>("container_provision_cmd", { instanceId });
}

export async function containerPreheat(pullImage = true) {
  return invokeOrThrow<import("./types").ContainerPreheatDto>("container_preheat_cmd", { pullImage });
}

export async function containerPullImage() {
  return invokeOrThrow<import("./types").ContainerImagePullDto>("container_pull_image_cmd");
}

export async function containerLayerState(instanceId: string) {
  return invokeOrThrow<import("./types").ContainerLayerStateDto>("container_layer_state_cmd", { instanceId });
}

export async function containerLayerApply(instanceId: string) {
  return invokeOrThrow<import("./types").ContainerLayerOpDto>("container_layer_apply_cmd", { instanceId });
}

export async function containerLayerDiscard(instanceId: string) {
  return invokeOrThrow<import("./types").ContainerLayerOpDto>("container_layer_discard_cmd", { instanceId });
}

export async function containerAgentWorkspace(instanceId: string) {
  return invokeOrThrow<import("./types").AgentWorkspaceDto>("container_agent_workspace_cmd", { instanceId });
}

export async function containerPreheatStatus() {
  return invokeOrThrow<import("./types").ContainerPreheatStatusDto>("container_preheat_status_cmd");
}

export async function containerPreheatEx(opts?: { pullImage?: boolean; forcePull?: boolean }) {
  return invokeOrThrow<import("./types").ContainerPreheatDto>("container_preheat_ex_cmd", {
    pullImage: opts?.pullImage ?? true,
    forcePull: opts?.forcePull ?? false,
  });
}

export async function containerMirrorProfiles() {
  return (
    (await invokeSafe<import("./types").ContainerMirrorProfileDto[]>("container_mirror_profiles_cmd")) ?? []
  );
}

export async function downloadMirrorCandidates() {
  return (
    (await invokeSafe<import("./types").DownloadMirrorCandidateDto[]>("download_mirror_candidates_cmd")) ?? []
  );
}

export async function probeDownloadMirrors() {
  return invokeOrThrow<import("./types").DownloadMirrorProbeDto[]>("probe_download_mirrors_cmd");
}

export async function registryMirrorCandidates() {
  return (
    (await invokeSafe<import("./types").RegistryMirrorCandidateDto[]>("registry_mirror_candidates_cmd")) ?? []
  );
}

export async function probeRegistryMirrors() {
  return invokeOrThrow<import("./types").RegistryMirrorProbeDto[]>("probe_registry_mirrors_cmd");
}

export async function containerRepair(opts?: {
  preferChinaMirrors?: boolean | null;
  reprovision?: boolean;
}) {
  return invokeOrThrow<import("./types").ContainerRepairDto>("container_repair_cmd", {
    preferChinaMirrors: opts?.preferChinaMirrors ?? null,
    reprovision: opts?.reprovision ?? true,
  });
}

export async function containerExportEnv(
  instanceId: string,
  opts?: { pack?: boolean; notes?: string | null },
) {
  return invokeOrThrow<import("./types").EnvExportResultDto>("container_export_env_cmd", {
    instanceId,
    pack: opts?.pack ?? false,
    notes: opts?.notes ?? null,
  });
}

export async function containerImportEnv(
  path: string,
  opts?: { targetInstanceId?: string | null; preferChinaMirrors?: boolean | null },
) {
  return invokeOrThrow<import("./types").EnvImportResultDto>("container_import_env_cmd", {
    path,
    targetInstanceId: opts?.targetInstanceId ?? null,
    preferChinaMirrors: opts?.preferChinaMirrors ?? null,
  });
}

export async function containerReadEnvManifest(path: string) {
  return invokeOrThrow<import("./types").EnvManifestDto>("container_read_env_manifest_cmd", { path });
}

// ——— Skin wardrobe ———

export async function wardrobeList() {
  return (
    (await invokeSafe<import("./types").WardrobeStateDto>("wardrobe_list_cmd")) ?? {
      entries: [],
      active_id: null,
    }
  );
}

export async function wardrobeSetActive(id: string | null) {
  return invokeOrThrow<import("./types").WardrobeStateDto>("wardrobe_set_active_cmd", { id });
}

export async function wardrobeUpload(opts: {
  skinPath: string;
  capePath?: string | null;
  name?: string | null;
  model?: string | null;
}) {
  return invokeOrThrow<import("./types").WardrobeEntryDto>("wardrobe_upload_cmd", {
    skinPath: opts.skinPath,
    capePath: opts.capePath ?? null,
    name: opts.name ?? null,
    model: opts.model ?? null,
  });
}

export async function wardrobeReplace(opts: {
  id: string;
  skinPath?: string | null;
  capePath?: string | null;
  name?: string | null;
  model?: string | null;
  clearCape?: boolean;
}) {
  return invokeOrThrow<import("./types").WardrobeEntryDto>("wardrobe_replace_cmd", {
    id: opts.id,
    skinPath: opts.skinPath ?? null,
    capePath: opts.capePath ?? null,
    name: opts.name ?? null,
    model: opts.model ?? null,
    clearCape: opts.clearCape ?? false,
  });
}

export async function wardrobeRemove(id: string) {
  return invokeOrThrow<import("./types").WardrobeStateDto>("wardrobe_remove_cmd", { id });
}

export async function skinFetchOfficial(query: string) {
  return invokeOrThrow<import("./types").ResolvedSkinDto>("skin_fetch_official_cmd", { query });
}

export async function skinFetchLittleSkin(name: string) {
  return invokeOrThrow<import("./types").ResolvedSkinDto>("skin_fetch_littleskin_cmd", { name });
}

export async function wardrobeImportOfficial(query: string, name?: string | null) {
  return invokeOrThrow<import("./types").WardrobeEntryDto>("wardrobe_import_official_cmd", {
    query,
    name: name ?? null,
  });
}

export async function wardrobeImportLittleSkin(player: string, name?: string | null) {
  return invokeOrThrow<import("./types").WardrobeEntryDto>("wardrobe_import_littleskin_cmd", {
    player,
    name: name ?? null,
  });
}
