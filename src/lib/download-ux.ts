/** Pure helpers for Download-page copy / CTA / filter (unit-tested). */

export type MarketTab = "mod" | "modpack" | "resourcepack" | "shader" | "datapack";

/** Primary install button on market cards — opens version-pick panel. */
export function marketInstallCta(_tab: MarketTab): string {
  return "安装";
}

export function marketNeedsInstance(tab: MarketTab): boolean {
  return tab !== "modpack";
}

export function marketEmptyTitle(query: string): { title: string; description: string } {
  const q = query.trim();
  if (q) {
    return {
      title: "咦？好像什么都没搜到…",
      description: "换个关键词试试，或者清空后再看热门",
    };
  }
  return {
    title: "咦？热门列表空空的…",
    description: "稍后再试，或换个来源看看",
  };
}

export function filterVersionIds(ids: readonly string[], filter: string): string[] {
  const f = filter.trim();
  if (!f) return [...ids];
  return ids.filter((id) => id.includes(f));
}

/** Prefer ready instances in home grid so not-ready cards don't steal the eye. */
export function sortInstancesReadyFirst<T extends { version_ready?: boolean | null }>(
  items: readonly T[],
): T[] {
  return [...items].sort((a, b) => Number(!!b.version_ready) - Number(!!a.version_ready));
}
