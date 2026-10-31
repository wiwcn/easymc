/** Pure helpers for Download-page copy / CTA / filter (unit-tested). */

export type MarketTab = "mod" | "modpack" | "resourcepack" | "shader" | "datapack";

/** Primary install button label — one clear verb per tab. */
export function marketInstallCta(tab: MarketTab): string {
  switch (tab) {
    case "modpack":
      return "安装整合包";
    case "mod":
      return "装进当前版本";
    case "resourcepack":
      return "下载资源包";
    case "shader":
      return "下载光影";
    case "datapack":
      return "下载数据包";
  }
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
