/** Payload from Tauri `install-progress` events (easymc-meta InstallProgress). */
export interface InstallProgressDto {
  stage: string;
  stage_label?: string;
  phase?: number;
  phase_total?: number;
  current: number;
  total: number | null;
  percent?: number | null;
  detail: string;
  skipped?: number;
  downloaded?: number;
}

const STAGE_LABELS: Record<string, string> = {
  meta: "元数据",
  client: "客户端",
  libraries: "依赖库",
  natives: "本地库",
  assets: "游戏资源",
  done: "完成",
  error: "失败",
};

export function stageLabel(p: InstallProgressDto): string {
  return p.stage_label || STAGE_LABELS[p.stage] || p.stage;
}

export function stagePercent(p: InstallProgressDto): number | null {
  if (p.percent != null && !Number.isNaN(p.percent)) return Math.max(0, Math.min(100, p.percent));
  if (p.total != null && p.total > 0) return Math.max(0, Math.min(100, (p.current / p.total) * 100));
  return null;
}

/** Overall rough percent across phases (equal weight). */
export function overallPercent(p: InstallProgressDto): number | null {
  const phases = p.phase_total ?? 0;
  const phase = p.phase ?? 0;
  if (phases <= 0 || phase <= 0) return stagePercent(p);
  if (p.stage === "done") return 100;
  if (p.stage === "error") return stagePercent(p);
  const local = stagePercent(p) ?? 0;
  const donePhases = Math.max(0, phase - 1);
  return Math.max(0, Math.min(100, ((donePhases + local / 100) / phases) * 100));
}

export function formatProgressLine(p: InstallProgressDto): string {
  const label = stageLabel(p);
  const phase =
    p.phase != null && p.phase_total != null ? `${p.phase}/${p.phase_total} ` : "";
  const pct = stagePercent(p);
  const pctStr = pct != null ? `${pct.toFixed(0)}%` : "—";
  const frac = p.total != null ? `${p.current}/${p.total}` : `${p.current}`;
  let extra = "";
  if ((p.skipped ?? 0) > 0 || (p.downloaded ?? 0) > 0) {
    extra = ` · 新下 ${p.downloaded ?? 0} / 跳过 ${p.skipped ?? 0}`;
  }
  return `[${phase}${label}] ${pctStr}  ${frac}  ${p.detail}${extra}`;
}
