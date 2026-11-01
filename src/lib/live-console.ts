/** Live launch-console helpers (filter / buffer / empty copy). Pure — unit-tested. */

export type LogSeverityFilter = "all" | "error" | "warn" | "info";

export const LIVE_CONSOLE_MAX_LINES = 4000;

/** Heuristic severity for Minecraft / JVM / launcher lines. */
export function lineSeverity(line: string): "error" | "warn" | "info" {
  const l = line.toLowerCase();
  if (
    /\/(fatal|error)\]/.test(l) ||
    /\b(fatal|severe)\b/.test(l) ||
    /exception/i.test(line) ||
    /\berror\b/.test(l) ||
    l.includes("caused by:") ||
    l.includes("crash report")
  ) {
    return "error";
  }
  if (/\/warn(ing)?\]/.test(l) || /\bwarn(ing)?\b/.test(l)) {
    return "warn";
  }
  return "info";
}

export function filterLogLines(
  lines: string[],
  query: string,
  severity: LogSeverityFilter = "all",
): string[] {
  const q = query.trim().toLowerCase();
  return lines.filter((line) => {
    if (severity !== "all" && lineSeverity(line) !== severity) return false;
    if (!q) return true;
    return line.toLowerCase().includes(q);
  });
}

/** Append one live line, capping buffer length (keeps the newest). */
export function appendLiveLog(
  prev: string[],
  line: string,
  maxKeep = LIVE_CONSOLE_MAX_LINES,
): string[] {
  const keep = Math.max(1, maxKeep);
  if (prev.length < keep) {
    return [...prev, line];
  }
  const start = prev.length - keep + 1;
  return [...prev.slice(start), line];
}

export function joinLogText(lines: string[]): string {
  return lines.join("\n");
}

export function consoleEmptyHint(opts: {
  hasFilter: boolean;
  launching?: boolean;
  failed?: boolean;
}): { title: string; detail: string } {
  if (opts.hasFilter) {
    return { title: "没有匹配的行", detail: "换个关键词试试，或清掉筛选。" };
  }
  if (opts.failed) {
    return {
      title: "启动没成功",
      detail: "下面若仍是空的，可让助手诊断，或到版本设置上传 latest.log。",
    };
  }
  if (opts.launching) {
    return { title: "正在等日志…", detail: "游戏刚启动时可能要几秒才有输出。" };
  }
  return {
    title: "还没有日志",
    detail: "点「开始游戏」后，这里会实时显示启动与运行输出。",
  };
}

/** True when the scrollport is near the bottom (sticky follow). */
export function shouldStickToBottom(
  el: { scrollTop: number; clientHeight: number; scrollHeight: number },
  slack = 48,
): boolean {
  return el.scrollTop + el.clientHeight >= el.scrollHeight - slack;
}
