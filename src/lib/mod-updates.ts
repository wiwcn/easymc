/** Pure helpers for batch mod update UI (selectable filter, labels, apply payload). */

import type {
  ModUpdateApplyItemDto,
  ModUpdateCandidateDto,
  ModUpdateStatusId,
} from "../types";

export function statusLabelZh(status: ModUpdateStatusId | string): string {
  switch (status) {
    case "has_update":
      return "有更新";
    case "up_to_date":
      return "已是最新";
    case "no_compatible":
      return "无兼容版本";
    case "unknown_source":
      return "未知来源";
    case "cf_needs_key":
      return "需 CurseForge Key";
    case "lookup_failed":
      return "检查失败";
    default:
      return String(status);
  }
}

export function isSelectableUpdate(c: ModUpdateCandidateDto): boolean {
  return c.status === "has_update" && !!c.source && !!c.project_id;
}

export function selectableCandidates(
  candidates: ModUpdateCandidateDto[],
): ModUpdateCandidateDto[] {
  return candidates.filter(isSelectableUpdate);
}

export function versionArrowLabel(c: ModUpdateCandidateDto, pickedId?: string | null): string {
  const cur = (c.current_version_number || c.current_version_id || "当前").trim();
  let next = c.latest_version_number || c.latest_version_id || "";
  if (pickedId && c.alternates?.length) {
    const alt = c.alternates.find((a) => a.id === pickedId);
    if (alt) next = alt.version_number || alt.name || pickedId;
  }
  if (!next) return cur;
  return `${cur} → ${next}`;
}

export function failReasonLabelZh(reason: string): string {
  switch (reason) {
    case "install_failed":
      return "安装失败";
    case "no_update":
      return "无需更新";
    case "unknown_source":
      return "未知来源";
    case "cf_needs_key":
      return "缺少 CurseForge Key";
    case "network":
      return "网络错误";
    default:
      return reason || "其他";
  }
}

export function buildApplyItems(
  candidates: ModUpdateCandidateDto[],
  selectedFilenames: Iterable<string>,
  versionPicks: Record<string, string | undefined | null>,
): ModUpdateApplyItemDto[] {
  const want = new Set(selectedFilenames);
  const out: ModUpdateApplyItemDto[] = [];
  for (const c of candidates) {
    if (!want.has(c.filename)) continue;
    if (!isSelectableUpdate(c)) continue;
    const pick = versionPicks[c.filename];
    out.push({
      filename: c.filename,
      source: c.source!,
      project_id: c.project_id!,
      version_id: pick || c.latest_version_id || null,
    });
  }
  return out;
}

export function defaultSelectedFilenames(candidates: ModUpdateCandidateDto[]): string[] {
  return selectableCandidates(candidates).map((c) => c.filename);
}

export function changelogPreview(text?: string | null, max = 160): string {
  if (!text) return "";
  const one = text.replace(/\s+/g, " ").trim();
  if (one.length <= max) return one;
  return one.slice(0, max - 1) + "…";
}
