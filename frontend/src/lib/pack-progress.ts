import type { ProgressEvent } from "../bindings";

/**
 * Overall-progress ranges per pack phase, in percent. The pipeline runs
 * phases in this order (clone → walk → tokenize → secret scan → compress
 * → emit); each phase's progress maps linearly into its range so the bar
 * reads as one continuous fill even though the phases report independent
 * 0–100 percentages. Skipped phases (e.g. secret scan off) simply jump
 * the bar forward when the next phase reports.
 */
export const PHASE_RANGES = {
  /** GitHub targets only — precedes the walk. */
  cloning: [0, 10],
  walking: [0, 40],
  tokenizing: [40, 60],
  secretScanning: [60, 80],
  compressing: [80, 95],
  emitting: [95, 100],
} as const;

/** Total scanned-file count at which the walking phase reads ~50% done.
 * The walker reports a running count with no total, so we map it onto an
 * asymptote that approaches (never reaches) the end of the walking range
 * — the bar keeps moving on huge repos without overshooting. */
const WALK_HALFWAY_FILES = 500;

function scaleIntoRange(
  pct: number,
  [start, end]: readonly [number, number],
): number {
  const clamped = Math.min(100, Math.max(0, pct));
  return start + (clamped / 100) * (end - start);
}

/**
 * Map a single progress event to its overall-progress percent, or `null`
 * for events that carry no phase position (file batches, secret hits,
 * non-fatal errors, …).
 */
export function progressForEvent(e: ProgressEvent): number | null {
  switch (e.kind) {
    case "started":
      return 0;
    case "cloning":
      return scaleIntoRange(e.progress_pct, PHASE_RANGES.cloning);
    case "walking": {
      const n = Math.max(0, e.files_scanned);
      const [, end] = PHASE_RANGES.walking;
      return end * (n / (n + WALK_HALFWAY_FILES));
    }
    case "tokenizing":
      return scaleIntoRange(e.progress_pct, PHASE_RANGES.tokenizing);
    case "secretScanning":
      return scaleIntoRange(e.progress_pct, PHASE_RANGES.secretScanning);
    case "compressing":
      return scaleIntoRange(e.progress_pct, PHASE_RANGES.compressing);
    case "transformStart":
    case "transformDone":
      // Transforms run inside the compress phase; anchor at its start.
      return PHASE_RANGES.compressing[0];
    case "buildingOutput":
      return PHASE_RANGES.emitting[0];
    case "done":
      return 100;
    default:
      return null;
  }
}

/**
 * Overall progress for an event log: the maximum phase progress observed.
 * Taking the max (rather than the latest event) keeps the bar monotonic
 * even when interleaved events map backwards (e.g. a straggling `walking`
 * tick after tokenizing started) and is robust to the store's 500-event
 * cap trimming old entries.
 */
export function progressFromEvents(events: ProgressEvent[]): number {
  let max = 0;
  for (const e of events) {
    const p = progressForEvent(e);
    if (p !== null && p > max) max = p;
  }
  return max;
}
