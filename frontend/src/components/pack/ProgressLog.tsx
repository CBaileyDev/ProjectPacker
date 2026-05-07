import type { ProgressEvent } from "../../bindings";

export function ProgressLog({ events }: { events: ProgressEvent[] }) {
  const lines: string[] = events
    .map((e) => {
      if (e.kind === "started") return `▶ ${e.target_label}`;
      if (e.kind === "walking")
        return `  Walking… ${e.files_scanned} files scanned`;
      if (e.kind === "tokenizing") return `  Tokenizing… ${e.progress_pct}%`;
      if (e.kind === "secretScanning")
        return `  Secret scan… ${e.progress_pct}%`;
      if (e.kind === "compressing") return `  Compressing… ${e.progress_pct}%`;
      if (e.kind === "buildingOutput") return `  Building output…`;
      if (e.kind === "cloning") return `  Cloning repository…`;
      if (e.kind === "secretHit")
        return `  ⚠ Secret in ${e.path} (line ${e.line})`;
      if (e.kind === "done") return `✓ Done`;
      if (e.kind === "error") return `✗ Error: ${e.message}`;
      return null;
    })
    .filter((l): l is string => l !== null);

  return (
    <div className="rounded border border-zinc-700 bg-zinc-900 px-4 py-3">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">
        Progress
      </div>
      <div className="space-y-0.5 font-mono text-xs text-zinc-400">
        {lines.slice(-16).map((l, i) => (
          // Progress log is append-only and never reordered; the trailing-window
          // index is a stable identity for as long as the line is on screen.
          // biome-ignore lint/suspicious/noArrayIndexKey: append-only log
          <div key={i}>{l}</div>
        ))}
      </div>
    </div>
  );
}
