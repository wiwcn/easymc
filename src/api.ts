import type { AccountDto, AccountsState, AgentAskResponse, AppMode, ChangeProposalDto, ContentInstallResultDto, CurseForgeStatusDto, DiagnoseReportDto, ExportResultDto, ImportResultDto, InstanceDto, InstancesState, LoaderVersionItem, LocalEntryDto, LlmStatusDto, ModInstallResultDto, ModpackInstallResultDto, ModProjectDto, ModVersionDto, MsaStatusDto, PreferredLogin, ProjectTypeId, SettingsDto, VersionSummary } from "./types";
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
  return (
    (await invokeSafe<ModProjectDto[]>("search_mods_cmd", {
      source,
      query: query || null,
      gameVersion: gameVersion || null,
      loader: loader || null,
      limit,
    })) ?? []
  );
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

export async function listInstalledMods(instanceId?: string | null): Promise<string[]> {
  return (
    (await invokeSafe<string[]>("list_installed_mods_cmd", {
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


export async function agentLlmStatus(): Promise<LlmStatusDto> {
  return (
    (await invokeSafe<LlmStatusDto>("agent_llm_status")) ?? {
      configured: false,
      mode: "rules_only",
    }
  );
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


export async function getSettings(): Promise<SettingsDto> {
  return (
    (await invokeSafe<SettingsDto>("get_settings")) ?? {
      memory_mb: 2048,
      mode: "standard",
      onboarding_complete: true, // browser preview skips gate
      download_source: "official",
    }
  );
}

export async function updateSettings(patch: {
  mode?: AppMode | string;
  onboarding_complete?: boolean;
  preferred_login?: PreferredLogin | string | null;
  memory_mb?: number;
  msa_client_id?: string | null;
  clear_msa_client_id?: boolean;
  download_source?: string;
}): Promise<SettingsDto> {
  return invokeOrThrow<SettingsDto>("update_settings_cmd", { patch });
}

export async function completeOnboarding(
  mode: AppMode | string,
  preferredLogin?: PreferredLogin | string | null,
): Promise<SettingsDto> {
  return invokeOrThrow<SettingsDto>("complete_onboarding_cmd", {
    mode,
    preferredLogin: preferredLogin ?? null,
  });
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
  return (
    (await invokeSafe<ModProjectDto[]>("search_content_cmd", {
      source,
      projectType,
      query: query || null,
      gameVersion: gameVersion || null,
      loader: loader || null,
      limit,
    })) ?? []
  );
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
