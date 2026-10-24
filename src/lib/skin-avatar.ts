import type { AccountDto } from "../types";

/** True for empty / nil / offline-generated placeholder UUIDs that won't resolve on skin CDNs. */
function isUsableUuid(uuid: string | undefined | null): boolean {
  if (!uuid) return false;
  const u = uuid.trim().toLowerCase().replace(/-/g, "");
  if (!/^[0-9a-f]{32}$/.test(u)) return false;
  // all-zero / preview nils
  if (/^0+$/.test(u)) return false;
  return true;
}

/**
 * Build a transparent PNG head URL from a public skin CDN (mc-heads.net).
 * Prefer UUID for Microsoft (MSA); username for offline / skin-station.
 */
export function skinAvatarUrl(
  account: Pick<AccountDto, "kind" | "username" | "uuid"> | null | undefined,
  size = 64,
): string | null {
  if (!account) return null;
  const kind = (account.kind || "").toLowerCase();
  const name = (account.username || "").trim();
  const asciiName = /^[A-Za-z0-9_]{1,16}$/.test(name) ? name : null;

  if (kind === "microsoft" && isUsableUuid(account.uuid)) {
    return `https://mc-heads.net/avatar/${account.uuid}/${size}`;
  }
  // Offline / skin station: username first (CDN looks up by name); UUID fallback.
  if (asciiName) {
    return `https://mc-heads.net/avatar/${encodeURIComponent(asciiName)}/${size}`;
  }
  if (isUsableUuid(account.uuid)) {
    return `https://mc-heads.net/avatar/${account.uuid}/${size}`;
  }
  return null;
}

/** Classic Steve head — last-resort CDN URL when we have no account identity. */
export const STEVE_AVATAR_URL = "https://mc-heads.net/avatar/Steve/64";

export function accountKindLabel(kind: string): string {
  const k = (kind || "").toLowerCase();
  if (k === "microsoft") return "正版";
  if (k === "skin_station") return "皮肤站";
  return "离线";
}

export function accountKindBadgeClass(kind: string): "ms" | "skin" | "off" {
  const k = (kind || "").toLowerCase();
  if (k === "microsoft") return "ms";
  if (k === "skin_station") return "skin";
  return "off";
}
