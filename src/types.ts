export type NavId = "home" | "library" | "store" | "agent" | "share" | "settings";

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
