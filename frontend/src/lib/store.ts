import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { useShallow } from "zustand/react/shallow";
import type {
  PackOptions,
  PackResult,
  PackStats,
  ProgressEvent,
  TransformReport,
} from "./api";
import { batchEvents } from "./events";
import { tauriStoreAdapter } from "./persist-adapter";

type PackingStatus = "idle" | "running" | "done" | "error";

interface AppState {
  jobId: string | null;
  status: PackingStatus;
  events: ProgressEvent[];
  result: PackResult | null;
  /** Latest PackStats — populated live during a pack from `transformDone`
   * events (per-transform `bytesSaved`/`filesTouched` patched into
   * `transforms`) and from the terminal `done` event's `stats`. Persisted
   * across the final `setResult`, but cleared on `reset`. The Compression
   * panel reads `lastStats.transforms` for per-row savings chips. */
  lastStats: PackStats | null;
  options: PackOptions;
  setJob: (id: string) => void;
  pushEvent: (e: ProgressEvent) => void;
  /** Append a batch of events at once. The batch is first run through
   * `batchEvents` to collapse runs of consecutive `walking` ticks, then
   * appended and clamped to 500. Used by the 100ms flush buffer in
   * `usePackJob` so a high-throughput pack doesn't trigger one render per
   * file scanned. */
  pushEventsBatched: (events: ProgressEvent[]) => void;
  setResult: (r: PackResult) => void;
  reset: () => void;
  setOptions: (o: PackOptions) => void;
  /** Merge a partial update into options, reading current state at call
   * time. Use this from async handlers (pickFolder, drop) so a slow
   * dialog doesn't capture stale `options` and overwrite a recent edit
   * to a different field on save. */
  patchOptions: (patch: Partial<PackOptions>) => void;
}

const defaultOptions: PackOptions = {
  target: { kind: "folder", value: "" },
  goal: "",
  countTokens: true,
  tokenizerModel: "gpt-4o-mini",
  secretScan: true,
  compress: false,
  removeComments: false,
  // Lossless 4 (default on — match Rust `PackOptions::default`).
  dedupFiles: true,
  trimTrailingWs: true,
  collapseBlankLines: true,
  normalizeLineEndings: true,
  // Semantic 3 + TS type-only elider (default off — opt in).
  collapseLockfiles: false,
  collapseMinified: false,
  markGenerated: false,
  elideTypeOnlyExports: false,
  maxFileSizeKb: 1024,
  respectGitignore: true,
  customIgnorePatterns: [],
  protocolVersion: "grok-to-cc-v1",
  format: "xml",
  xmlSchema: "cxml",
};

/** Empty PackStats used as a seed for `lastStats` when the first
 * `transformDone` event arrives before any other stats are known. The UI
 * only reads `transforms` from this seed, but all fields are populated to
 * keep the type honest. */
function emptyStats(): PackStats {
  return {
    filesTotal: 0,
    filesIncluded: 0,
    filesSkipped: 0,
    bytesTotal: 0,
    tokensTotal: null,
    tokensPerModel: null,
    secretsFound: 0,
    durationMs: 0,
    walkMs: 0,
    processMs: 0,
    secretScanMs: null,
    tokenizeMs: null,
    emitMs: 0,
    transforms: [],
    transformPhaseMs: 0,
  };
}

/** Merge a `transformDone` event into `lastStats.transforms` — append a new
 * entry on first sight, replace in place on duplicate (the Rust side
 * shouldn't emit the same id twice in one run, but tolerate it so a buggy
 * pipeline doesn't double-count). Maps snake_case wire fields onto the
 * camelCase `TransformReport` shape; the inner `transformDone` payload is
 * raw serde (snake_case) while `TransformReport` is camelCased by specta. */
function applyTransformDone(
  stats: PackStats | null,
  ev: { id: string; bytes_saved: number; files_touched: number },
): PackStats {
  const base = stats ?? emptyStats();
  const entry: TransformReport = {
    id: ev.id,
    bytesSaved: ev.bytes_saved,
    filesTouched: ev.files_touched,
    elapsedMs: 0,
  };
  const idx = base.transforms.findIndex((t) => t.id === ev.id);
  const transforms =
    idx >= 0
      ? base.transforms.map((t, i) => (i === idx ? entry : t))
      : [...base.transforms, entry];
  return { ...base, transforms };
}

// Cap at 500. The UI only ever displays the last 16 (`ProgressLog.slice(-16)`)
// but we keep more so a debugger or future "show full log" feature has data
// to work with. Above 500 the array slice cost dominates.
const EVENT_CAP = 500;

/** Compute the next status given the current status and an incoming event. */
function nextStatus(prev: PackingStatus, e: ProgressEvent): PackingStatus {
  if (e.kind === "done") return "done";
  if (e.kind === "error") return "error";
  return prev;
}

export const useApp = create<AppState>()(
  persist(
    (set) => ({
      jobId: null,
      status: "idle",
      events: [],
      result: null,
      lastStats: null,
      options: defaultOptions,
      setJob: (id) =>
        set({
          jobId: id,
          status: "running",
          events: [],
          result: null,
          lastStats: null,
        }),
      pushEvent: (e) =>
        set((s) => {
          const base = {
            events: [...s.events, e].slice(-EVENT_CAP),
            status: nextStatus(s.status, e),
          };
          // `transformDone` and the terminal `done` both refresh
          // `lastStats` — the former patches a single entry into
          // `transforms` during the run; the latter overwrites with the
          // authoritative final PackStats. Anything else passes through.
          if (e.kind === "transformDone") {
            return { ...base, lastStats: applyTransformDone(s.lastStats, e) };
          }
          if (e.kind === "done") {
            return { ...base, lastStats: e.stats };
          }
          return base;
        }),
      pushEventsBatched: (incoming) =>
        set((s) => {
          if (incoming.length === 0) return s;
          // Stitch the existing tail together with the new batch before
          // running batchEvents — that way a `walking` event already at
          // the end of `s.events` gets collapsed into a `walking` first
          // event of `incoming`, instead of staying as two adjacent
          // walking entries across the boundary.
          const merged = batchEvents([...s.events, ...incoming]);
          let nextStat = s.status;
          let nextStats: PackStats | null = s.lastStats;
          for (const e of incoming) {
            nextStat = nextStatus(nextStat, e);
            if (e.kind === "transformDone") {
              nextStats = applyTransformDone(nextStats, e);
            } else if (e.kind === "done") {
              nextStats = e.stats;
            }
          }
          return {
            events: merged.slice(-EVENT_CAP),
            status: nextStat,
            lastStats: nextStats,
          };
        }),
      setResult: (r) => set({ result: r, lastStats: r.stats }),
      reset: () =>
        set({
          jobId: null,
          status: "idle",
          events: [],
          result: null,
          lastStats: null,
        }),
      setOptions: (o) => set({ options: o }),
      patchOptions: (patch) =>
        set((s) => ({ options: { ...s.options, ...patch } })),
    }),
    {
      name: "app-state",
      storage: createJSONStorage(() => tauriStoreAdapter),
      partialize: (state) => ({ options: state.options }) as Partial<AppState>,
      // Persisted state from v0.5 lacks the 8 new toggle fields; a naive
      // shallow merge would replace `defaultOptions` wholesale and leave
      // `dedupFiles` etc. undefined → all lossless defaults silently off.
      // Deep-merging `options` lets old installs inherit the new defaults
      // for fields they've never seen while preserving their existing
      // edits. New fields land via `defaultOptions`, persisted edits win
      // for fields the user has touched.
      merge: (persisted, current) => {
        const p = persisted as { options?: Partial<PackOptions> } | undefined;
        return {
          ...current,
          options: { ...current.options, ...(p?.options ?? {}) },
        };
      },
    },
  ),
);

// ── Selector hooks ──────────────────────────────────────────────────────────
// These return a stable shallow-equal slice so a component that only watches
// `options` won't re-render when an event lands. `useShallow` is the v5 way
// of doing this — `zustand/shallow` plus a selector function.

/** Subscribe to the editable options + their patcher only. */
export function usePackOptions() {
  return useApp(
    useShallow((s) => ({
      options: s.options,
      setOptions: s.setOptions,
      patchOptions: s.patchOptions,
    })),
  );
}

/** Subscribe to the live progress slice (status + events + result). */
export function usePackProgress() {
  return useApp(
    useShallow((s) => ({
      status: s.status,
      events: s.events,
      result: s.result,
      jobId: s.jobId,
    })),
  );
}

/** Subscribe to `lastStats` only. The Compression panel reads
 * `lastStats.transforms` to render per-row savings chips during and after
 * a pack — a dedicated selector keeps unrelated re-renders from triggering
 * a tree rebuild of the toggle list. */
export function useLastStats(): PackStats | null {
  return useApp((s) => s.lastStats);
}
