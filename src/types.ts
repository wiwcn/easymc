export type NavId = "home" | "download" | "instances" | "multiplayer" | "accounts" | "settings" | "more" | "agent";

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
  category?: string | null;
  memory_mb?: number | null;
  java_path?: string | null;
  width?: number | null;
  height?: number | null;
  jvm_args?: string | null;
}

export type ProjectTypeId = "mod" | "modpack" | "resourcepack" | "shader" | "datapack";

export interface LocalEntryDto {
  name: string;
  is_dir: boolean;
  size: number;
  modified?: string | null;
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

export interface AgentAskResponse {
  mode: string;
  reply_zh: string;
  report?: DiagnoseReportDto | null;
  action?: AgentActionDto | null;
  llm: LlmStatusDto;
}

export interface ExportResultDto {
  path: string;
  mode: "metadata" | "pack" | string;
  mod_count: number;
  bytes: number;
  message_zh: string;
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

export interface SettingsDto {
  msa_client_id?: string | null;
  preferred_java?: string | null;
  memory_mb: number;
  curseforge_api_key?: string | null;
  mode: AppMode;
  onboarding_complete: boolean;
  preferred_login?: PreferredLogin | string | null;
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
