import type { InstanceDto } from "../types";

/** True when `icon` / `icon_url` is already a usable image URL. */
export function isUsableIconUrl(s?: string | null): boolean {
  if (!s) return false;
  const t = s.trim();
  return (
    t.startsWith("https://") ||
    t.startsWith("http://") ||
    t.startsWith("data:image/") ||
    t.startsWith("/")
  );
}

/**
 * PCL-style display URL for a version/instance tile:
 * 1. Resolved `icon_url` from backend (pack.png / Modrinth / CF)
 * 2. Stored `icon` when it is already an http(s)/data URL
 * 3. Loader default under `/instance-icons/` (grass for vanilla/fabric/quilt; forge/neoforge marks)
 */
export function instanceIconUrl(inst: InstanceDto): string {
  if (isUsableIconUrl(inst.icon_url)) return inst.icon_url!.trim();
  if (isUsableIconUrl(inst.icon)) return inst.icon!.trim();
  const loader = String(inst.loader || "vanilla").toLowerCase();
  if (loader === "forge") return "/instance-icons/forge.png";
  if (loader === "neoforge") return "/instance-icons/neoforge.png";
  if (loader === "fabric") return "/instance-icons/fabric.png";
  if (loader === "quilt") return "/instance-icons/quilt.png";
  return "/instance-icons/grass.png";
}
