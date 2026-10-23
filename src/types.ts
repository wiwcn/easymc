export type NavId = "home" | "library" | "store" | "agent" | "share" | "settings";

export interface InstanceDto {
  id: string;
  name: string;
  version: string | { 0: string };
  loader: string;
  description?: string | null;
  last_played?: string | null;
}

export function versionId(v: InstanceDto["version"]): string {
  return typeof v === "string" ? v : v[0] ?? String(v);
}
