import type { PackStats } from "../../bindings";

export function PhaseBreakdown({ stats }: { stats: PackStats }) {
  // Helpers — render `Some(n)` as `Nms` and `None` as an em-dash.
  const opt = (n: number | null | undefined): string =>
    typeof n === "number" ? `${n}ms` : "—";
  const req = (n: number): string => `${n}ms`;

  return (
    <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 px-1 text-xs font-mono text-zinc-500">
      <span>walk {req(stats.walkMs)}</span>
      <span>· process {req(stats.processMs)}</span>
      <span>· secret-scan {opt(stats.secretScanMs)}</span>
      <span>· tokenize {opt(stats.tokenizeMs)}</span>
      <span>· emit {req(stats.emitMs)}</span>
    </div>
  );
}
