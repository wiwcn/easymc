export type NavId = "home" | "download" | "instances" | "multiplayer" | "accounts" | "wardrobe" | "settings" | "more" | "agent";

export type QuickPlayMode = "world" | "server" | "realms";

/** Skip title screen on launch (Minecraft 1.20+ Quick Play). */
export interface QuickPlayDto {
  type: QuickPlayMode;
  name?: string;
  address?: string;
  id?: string;
}

export interface MclogsUploadResultDto {
  id: string;
  url: string;
  raw?: string | null;
  message_zh: string;
}

export interface InstanceDto {
  id: string;
  name: string;
  version: string | { 0: string };
  loader: string;
  loader_version?: string | null;
  description?: string | null;
  last_played?: string | null;
  game_dir?: string;
  account_id?: string | null;
  version_ready?: boolean;
  created_at?: string;
  icon?: string | null;
  /** Resolved pack / marketplace icon (`http(s)` or `data:`). Prefer over `icon` for display. */
  icon_url?: string | null;
  category?: string | null;
  memory_mb?: number | null;
  java_path?: string | null;
  width?: number | null;
  height?: number | null;
  jvm_args?: string | null;
  /** Direct-join world/server/Realms on launch. */
  quick_play?: QuickPlayDto | null;
  /** Present when this version is container-backed (Podman). */
  container?: ContainerMetaDto | null;
}

export interface ContainerVolumeDto {
  host: string;
  container: string;
  mode: string;
}

export interface ContainerMetaDto {
  runtime: string;
  container_id?: string | null;
  image?: string | null;
  volumes?: ContainerVolumeDto[];
  provisioned?: boolean;
}

export type ContainerProbeStatus = "not_installed" | "installed_not_ready" | "ready" | "unsupported";

export interface ContainerRuntimeProbeDto {
  status: ContainerProbeStatus | string;
  podman?: { path: string; version: string; ready: boolean; detail_zh?: string | null } | null;
  message_zh: string;
  platform: string;
}

export interface ContainerSetupStepDto {
  action: string;
  title_zh: string;
  detail_zh: string;
  command?: string | null;
  url?: string | null;
}

export interface ContainerSetupPlanDto {
  actions: ContainerSetupStepDto[];
  probe: ContainerRuntimeProbeDto;
  summary_zh: string;
}

export interface ChinaRegistriesResultDto {
  path: string;
  wrote: boolean;
  message_zh: string;
}

export interface ContainerSetupResultDto {
  ok: boolean;
  probe: ContainerRuntimeProbeDto;
  registries?: ChinaRegistriesResultDto | null;
  messages_zh: string[];
  pending: ContainerSetupStepDto[];
}

export interface ContainerVersionSummaryDto {
  instance_id: string;
  name: string;
  version: string;
  loader: string;
  runtime: string;
  container_id?: string | null;
  image?: string | null;
  volumes: ContainerVolumeDto[];
  provisioned: boolean;
}

export type ProjectTypeId = "mod" | "modpack" | "resourcepack" | "shader" | "datapack";

export interface LocalEntryDto {
  name: string;
  is_dir: boolean;
  size: number;
  modified?: string | null;
  icon_url?: string | null;
}

export interface InstalledModEntryDto {
  filename: string;
  disabled: boolean;
  size: number;
  icon_url?: string | null;
  source?: string | null;
  project_id?: string | null;
}

export interface ContentInstallResultDto {
  project_type: ProjectTypeId | string;
  project_name: string;
  version_number: string;
  filename: string;
  path: string;
  skipped: boolean;
}

export interface ModpackInstallResultDto {
  instance_id: string;
  instance_name: string;
  game_version: string;
  loader: string;
  loader_version?: string | null;
  files_downloaded: number;
  files_failed: string[];
  overrides: number;
  message_zh: string;
}

export interface InstancesState {
  instances: InstanceDto[];
  active_id: string | null;
}

export interface AccountDto {
  id: string;
  kind: string;
  username: string;
  uuid: string;
}

export interface AccountsState {
  accounts: AccountDto[];
  active_id: string | null;
}

export interface VersionSummary {
  id: string;
  /** Backend serializes as `type` (piston-meta) or `version_type`. */
  type?: string;
  version_type?: string;
  url: string;
  installed?: boolean;
}

export function versionId(v: InstanceDto["version"]): string {
  return typeof v === "string" ? v : (v as { 0: string })[0] ?? String(v);
}

export function versionType(v: VersionSummary): string {
  return v.version_type ?? v.type ?? "release";
}

export type LoaderId = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";

export interface LoaderVersionItem {
  version: string;
  stable: boolean;
  intermediary: string;
}

export function loaderLabel(loader: string): string {
  switch ((loader || "vanilla").toLowerCase()) {
    case "fabric":
      return "Fabric";
    case "quilt":
      return "Quilt";
    case "forge":
      return "Forge";
    case "neoforge":
      return "NeoForge";
    default:
      return "原版";
  }
}

/** Player-facing primary label: `1.21.1 · Fabric` / `1.21.1 · 原版` (not custom display names). */
export function playerVersionLabel(mcVersion: string, loader: string): string {
  const ver = (mcVersion || "").trim() || "?";
  return `${ver} · ${loaderLabel(loader)}`;
}

export function instancePlayerLabel(i: { version: InstanceDto["version"]; loader: string }): string {
  return playerVersionLabel(versionId(i.version), i.loader);
}

export type ModSourceId = "modrinth" | "curseforge";

export interface ModProjectDto {
  id: string;
  slug: string;
  name: string;
  description: string;
  source: ModSourceId;
  downloads: number;
  author?: string | null;
  icon_url?: string | null;
  categories: string[];
}

export interface ModVersionDto {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  downloads: number;
  date_published?: string | null;
  files: { filename: string; url: string; size?: number | null; primary: boolean; sha1?: string | null }[];
}

export interface ModInstallResultDto {
  source: ModSourceId;
  project_id: string;
  project_name: string;
  version_id: string;
  version_number: string;
  filename: string;
  path: string;
  skipped: boolean;
}

export type DepStatusId = "ok" | "missing" | "conflict" | "provided" | "disabled" | "unknown";
export type DepKindId = "required" | "optional" | "incompatible";

export interface DepNodeDto {
  mod_id: string;
  display_name: string;
  version?: string | null;
  filename?: string | null;
  status: DepStatusId;
  message_zh?: string | null;
  version_range?: string | null;
  dep_kind?: DepKindId | null;
  fetchable: boolean;
  children: DepNodeDto[];
}

export interface ModMetaDto {
  mod_id: string;
  version: string;
  name?: string | null;
  filename: string;
  disabled: boolean;
  kind: string;
  depends: { mod_id: string; version_range?: string | null; kind: DepKindId }[];
  breaks: { mod_id: string; version_range?: string | null; kind: DepKindId }[];
}

export interface DepGraphReportDto {
  roots: DepNodeDto[];
  installed: ModMetaDto[];
  missing_ids: string[];
  conflict_messages: string[];
  summary_zh: string;
}

export interface FetchMissingResultDto {
  attempted: string[];
  installed: ModInstallResultDto[];
  failed: { mod_id: string; error_zh: string; reason?: string | null; tried_slug?: string | null }[];
  message_zh: string;
}

export interface CurseForgeStatusDto {
  configured: boolean;
  env: string;
  hint: string;
  source?: "env" | "settings" | "none" | string;
  console_url?: string;
  key_len?: number;
}

export interface LlmStatusDto {
  configured: boolean;
  model?: string | null;
  base_url_host?: string | null;
  mode: string;
}

export interface LlmConfigViewDto {
  path?: string | null;
  base_url: string;
  model: string;
  api_key_set: boolean;
  api_key_hint: string;
  configured: boolean;
}

export interface AgentFinding {
  id: string;
  severity: "info" | "warn" | "error" | string;
  title: string;
  summary: string;
  fixes: string[];
  action?: string | null;
  evidence?: string | null;
}

export interface DiagnoseReportDto {
  ok: boolean;
  source_label: string;
  findings: AgentFinding[];
  summary_zh: string;
  log_excerpt: string;
  log_chars: number;
  sources: string[];
}

export interface AgentActionDto {
  ok: boolean;
  action: string;
  message_zh: string;
  path?: string | null;
  value?: unknown;
}

export interface ProposeItemDto {
  key: string;
  label_zh: string;
  current: string;
  proposed: string;
  delta_zh: string;
}

export interface ChangeProposalDto {
  id: string;
  title_zh: string;
  tag_zh?: string | null;
  subtitle_zh?: string | null;
  items: ProposeItemDto[];
  apply_payload: {
    instance_id?: string | null;
    memory_mb?: number | null;
    jvm_args?: string | null;
    clear_jvm_args?: boolean;
  };
  undo_snapshot: {
    instance_id?: string | null;
    settings_memory_mb: number;
    instance_memory_mb?: number | null;
    instance_had_memory: boolean;
    instance_jvm_args?: string | null;
    instance_had_jvm: boolean;
  };
}


export type ToolStatus = "pending" | "running" | "ok" | "error";

export interface ToolArgDto {
  key: string;
  label_zh: string;
  value: string;
}

export interface ToolUiActionDto {
  id: string;
  label_zh: string;
}

/** Normalized tool invocation for chat tool cards (rules + cloud paths). */
export interface ToolCallDto {
  id: string;
  name: string;
  title_zh: string;
  status: ToolStatus;
  args: ToolArgDto[];
  result_zh?: string | null;
  path?: string | null;
  payload?: unknown;
  ui_actions: ToolUiActionDto[];
  tag_zh?: string | null;
}

export interface AgentAskResponse {
  mode: string;
  reply_zh: string;
  report?: DiagnoseReportDto | null;
  action?: AgentActionDto | null;
  /** Last / primary tool card (compat). */
  tool?: ToolCallDto | null;
  /** All tool cards from a multi-step agent loop. */
  tools?: ToolCallDto[];
  llm: LlmStatusDto;
}

export interface ExportResultDto {
  path: string;
  mode: "metadata" | "pack" | "mrpack" | string;
  mod_count: number;
  bytes: number;
  message_zh: string;
  resolved?: number | null;
  overrides?: number | null;
}

export interface ImportResultDto {
  instance_id: string;
  instance_name: string;
  mods_copied: number;
  mods_downloaded: number;
  mods_missing: string[];
  memory_applied?: number | null;
  message_zh: string;
  path_loaded: string;
}


export type AppMode = "agent" | "standard";

export type PreferredLogin = "microsoft" | "offline" | "skin_station";

export type DownloadSourceId = "official" | "bmclapi";

export interface SettingsDto {
  msa_client_id?: string | null;
  preferred_java?: string | null;
  memory_mb: number;
  curseforge_api_key?: string | null;
  mode: AppMode;
  onboarding_complete: boolean;
  preferred_login?: PreferredLogin | string | null;
  /** Game file download source: official Mojang or BMCLAPI mirror. */
  download_source?: DownloadSourceId | string;
  container_mode_enabled?: boolean;
  container_china_mirrors?: boolean;
  /** daocloud | dockerproxy | onems | official */
  container_mirror_profile?: string;
  container_setup_done?: boolean;
  /** Container mode: mutating saves/ needs Agent propose (default true). */
  container_saves_require_propose?: boolean;
  /** Auto-preheat when container mode enabled (Slice E). */
  container_auto_preheat?: boolean;
}

export interface MsaStatusDto {
  configured: boolean;
  using_temporary_client_id: boolean;
  oauth_ready: boolean;
  message_zh: string;
}

export interface DeviceCodeDto {
  user_code: string;
  device_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

export interface ContainerImagePullDto {
  image: string;
  pulled: boolean;
  already_present: boolean;
  message_zh: string;
}

export interface ContainerPreheatDto {
  ok: boolean;
  machine_started: boolean;
  image?: ContainerImagePullDto | null;
  probe_status: string;
  messages_zh: string[];
  warmed_at?: number | null;
}

export interface ContainerPreheatStatusDto {
  probe_status: string;
  machine_ready: boolean;
  image_ready: boolean;
  warm: boolean;
  last_preheat_at?: number | null;
  seconds_since_preheat?: number | null;
  message_zh: string;
}

export interface ContainerMirrorProfileDto {
  id: string;
  label_zh: string;
  prefer_china: boolean;
  sample_image: string;
  probe_host?: string;
}

export interface DownloadMirrorCandidateDto {
  id: string;
  label_zh: string;
  region_zh: string;
  download_source: string;
  probe_url: string;
  host_hint: string;
}

export interface DownloadMirrorProbeDto {
  id: string;
  label_zh: string;
  region_zh: string;
  download_source: string;
  ok: boolean;
  latency_ms?: number | null;
  error_zh?: string | null;
  recommended: boolean;
}

export interface RegistryMirrorCandidateDto {
  id: string;
  label_zh: string;
  region_zh: string;
  prefer_china: boolean;
  probe_host: string;
  probe_url?: string;
  sample_image: string;
}

export interface RegistryMirrorProbeDto {
  id: string;
  label_zh: string;
  region_zh: string;
  prefer_china: boolean;
  ok: boolean;
  latency_ms?: number | null;
  error_zh?: string | null;
  recommended: boolean;
  sample_image: string;
}

export interface ContainerRepairDto {
  ok: boolean;
  probe: ContainerRuntimeProbeDto;
  registries?: ChinaRegistriesResultDto | null;
  preheat?: ContainerPreheatDto | null;
  image?: ContainerImagePullDto | null;
  reprovisioned: number;
  messages_zh: string[];
}

export interface EnvManifestDto {
  format: string;
  format_version: number;
  mode: "metadata" | "pack" | string;
  exported_at: string;
  notes?: string | null;
  instance: { name: string; version: string; loader: string; loader_version?: string | null };
  container: { runtime: string; image?: string | null; prefer_china_mirrors: boolean };
  layer?: { dirs: string[]; include_saves: boolean; mod_filenames: string[] };
  settings_hint?: { container_china_mirrors: boolean; container_saves_require_propose: boolean } | null;
}

export interface EnvExportResultDto {
  path: string;
  mode: "metadata" | "pack" | string;
  bytes: number;
  message_zh: string;
}

export interface EnvImportResultDto {
  instance: InstanceDto;
  mode: "metadata" | "pack" | string;
  layer_files: number;
  message_zh: string;
}

export interface ContainerProvisionDto {
  instance: InstanceDto;
  image_pulled: boolean;
  message_zh: string;
}

export interface ContainerLayerStateDto {
  instance_id: string;
  baseline_at?: string | null;
  applied_at?: string | null;
  dirty: boolean;
  rev: number;
  message_zh: string;
}

export interface ContainerLayerOpDto {
  ok: boolean;
  state: ContainerLayerStateDto;
  message_zh: string;
}

export interface AgentWorkspaceDto {
  instance_id: string;
  kind: "native" | "container" | string;
  root: string;
  container_id?: string | null;
  image?: string | null;
  saves_require_propose: boolean;
  layer?: ContainerLayerStateDto | null;
  message_zh: string;
}

/** Steve / Alex / auto — wardrobe + skinview3d. */
export type SkinModelId = "classic" | "slim" | "default" | "auto-detect";

export interface WardrobeEntryDto {
  id: string;
  name: string;
  model: SkinModelId | string;
  source: string;
  source_label: string;
  has_cape: boolean;
  skin_data_url: string;
  cape_data_url?: string | null;
  account_id?: string | null;
  created_at: string;
}

export interface WardrobeStateDto {
  entries: WardrobeEntryDto[];
  active_id: string | null;
}

export interface StagedActiveSkinDto {
  id: string;
  name: string;
  model: string;
  skin_path: string;
  cape_path?: string | null;
  csl_skin_path?: string | null;
}

export interface ResolvedSkinDto {
  name: string;
  uuid?: string | null;
  model: string;
  skin_url: string;
  cape_url?: string | null;
  source: string;
  source_label: string;
  skin_data_url?: string | null;
  cape_data_url?: string | null;
}
