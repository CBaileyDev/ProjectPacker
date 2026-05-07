import type { PackStats } from "../../bindings";
import { fmtBytes, fmtNum } from "../../lib/format";

export function StatsBar({ stats }: { stats: PackStats }) {
  return (
    <div className="flex flex-wrap gap-x-6 gap-y-2 rounded border border-zinc-700 bg-zinc-800/50 px-4 py-3 text-sm">
      <span>
        <span className="text-zinc-400">Files </span>
        <span className="font-medium text-zinc-100">{stats.filesIncluded}</span>
        <span className="text-zinc-500"> / {stats.filesTotal}</span>
      </span>
      <span>
        <span className="text-zinc-400">Skipped </span>
        <span className="font-medium text-zinc-100">{stats.filesSkipped}</span>
      </span>
      <span>
        <span className="text-zinc-400">Size </span>
        <span className="font-medium text-zinc-100">
          {fmtBytes(stats.bytesTotal)}
        </span>
      </span>
      {stats.tokensTotal != null && (
        <span>
          <span className="text-zinc-400">Tokens </span>
          <span className="font-medium text-zinc-100">
            {fmtNum(stats.tokensTotal)}
          </span>
        </span>
      )}
      {stats.secretsFound > 0 && (
        <span className="font-medium text-amber-400">
          ⚠ {stats.secretsFound} secret{stats.secretsFound !== 1 ? "s" : ""}{" "}
          detected
        </span>
      )}
      <span>
        <span className="text-zinc-400">Time </span>
        <span className="font-medium text-zinc-100">{stats.durationMs}ms</span>
      </span>
    </div>
  );
}
