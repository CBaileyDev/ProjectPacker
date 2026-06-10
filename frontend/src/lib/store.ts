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
import { defaultOptions } from "./default-options";
import { batchEvents } from "./events";
import { tauriStoreAdapter } from "./persist-adapter";

type PackingStatus = "idle" | "running" | "done" | "error" | "cancelled";

export type Moment = "home" | "packing" | "results" | "bridge";
export type SheetId = "github" | "settings";

export interface RecentTarget {
  kind: "folder" | "github";
  value: string;
}

/** Moment transition applied when the pack status CHANGES. `done` lands on
 * Results; a fatal error returns Home (the error surfaces as a banner).
 * Statuses handled here are the ones foldEvent can produce; running→packing
 * and cancelled→home are set directly in setJob/markCancelled. */
function momentAfterStatus(prev: Moment, status: PackingStatus): Moment {
  if (status === "done") return "results";
  if (status === "error") return "home";
  return prev;
}

const RECENTS_CAP = 5;

function pushRecent(
  recents: RecentTarget[],
  target: RecentTarget,
): RecentTarget[] {
  if (target.value.length === 0) return recents;
  return [
    { kind: target.kind, value: target.value },
    ...recents.filter(
      (r) => !(r.kind === target.kind && r.value === target.value),
    ),
  ].slice(0, RECENTS_CAP);
}

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
  /** Plan markdown pasted into the Bridge tab. Lives in the store (not
   * component state) because tab content unmounts on tab switch — a large
   * pasted plan must survive a round-trip to Settings. Deliberately NOT
   * persisted (see `partialize`): a pasted plan should not outlive the
   * app session. */
  bridgePlanMd: string;
  /** Which full-screen view is showing. Driven by user navigation and by
   * pack-status transitions (setJob → packing, done → results,
   * error/cancel → home). Never persisted — the app always boots Home. */
  moment: Moment;
  /** Overlay sheet (GitHub / Settings) — orthogonal to `moment`. */
  activeSheet: SheetId | null;
  /** Set by the global mod+k handler; consumed (and cleared) by
   * TargetSection once it's mounted — survives the moment-swap animation
   * delay that breaks a one-shot rAF focus. Transient UI state, never
   * persisted. */
  pendingTargetFocus: boolean;
  /** Whether Home's Advanced options panel is expanded. Persisted. */
  advancedOpen: boolean;
  /** Last 5 pack targets, most recent first. Persisted. */
  recentTargets: RecentTarget[];
  setMoment: (m: Moment) => void;
  setSheet: (s: SheetId | null) => void;
  setPendingTargetFocus: (v: boolean) => void;
  setAdvancedOpen: (v: boolean) => void;
  setBridgePlanMd: (md: string) => void;
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
  /** Mark a running pack as user-cancelled. No-op unless a pack is in
   * flight, so a `done` that races the cancel click wins. Used by
   * `usePackJob.cancel` — cancellation is not an error, so it must not
   * route through the `error` event path. */
  markCancelled: () => void;
  /** Merge a partial update into options, reading current state at call
   * time. Use this from async handlers (pickFolder, drop) so a slow
   * dialog doesn't capture stale `options` and overwrite a recent edit
   * to a different field on save. */
  patchOptions: (patch: Partial<PackOptions>) => void;
}

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

/** Compute the next status given the current status and an incoming event.
 * Only a FATAL error is terminal — non-fatal error events are informational
 * (use-pack-job keeps streaming after them) and must not flip the status,
 * which would also yank the moment machine back Home mid-pack. */
function nextStatus(prev: PackingStatus, e: ProgressEvent): PackingStatus {
  if (e.kind === "done") return "done";
  if (e.kind === "error" && e.fatal) return "error";
  return prev;
}

/** Fold a single progress event into the `{status, lastStats}` pair —
 * the one place the transition rules live. `transformDone` patches a
 * single entry into `lastStats.transforms` during the run; the terminal
 * `done` overwrites with the authoritative final PackStats. Anything
 * else passes through. `pushEvent` applies this once; `pushEventsBatched`
 * folds it over the incoming batch. */
function foldEvent(
  acc: { status: PackingStatus; lastStats: PackStats | null },
  e: ProgressEvent,
): { status: PackingStatus; lastStats: PackStats | null } {
  const status = nextStatus(acc.status, e);
  if (e.kind === "transformDone") {
    return { status, lastStats: applyTransformDone(acc.lastStats, e) };
  }
  if (e.kind === "done") {
    return { status, lastStats: e.stats };
  }
  return { status, lastStats: acc.lastStats };
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
      bridgePlanMd: "",
      moment: "home",
      activeSheet: null,
      pendingTargetFocus: false,
      advancedOpen: false,
      recentTargets: [],
      setMoment: (m) => set({ moment: m }),
      setSheet: (sheet) => set({ activeSheet: sheet }),
      setPendingTargetFocus: (v) => set({ pendingTargetFocus: v }),
      setAdvancedOpen: (v) => set({ advancedOpen: v }),
      setBridgePlanMd: (md) => set({ bridgePlanMd: md }),
      setJob: (id) =>
        set((s) => ({
          jobId: id,
          status: "running",
          events: [],
          result: null,
          lastStats: null,
          moment: "packing",
          recentTargets: pushRecent(s.recentTargets, s.options.target),
        })),
      pushEvent: (e) =>
        set((s) => {
          const { status, lastStats } = foldEvent(
            { status: s.status, lastStats: s.lastStats },
            e,
          );
          return {
            events: [...s.events, e].slice(-EVENT_CAP),
            status,
            lastStats,
            moment:
              status !== s.status
                ? momentAfterStatus(s.moment, status)
                : s.moment,
          };
        }),
      pushEventsBatched: (incoming) =>
        set((s) => {
          if (incoming.length === 0) return s;
          // Single point where batchEvents is applied (usePackJob's flush
          // buffer passes its raw buffer through untouched). Stitch the
          // existing tail together with the new batch before running
          // batchEvents — that way a `walking` event already at the end
          // of `s.events` gets collapsed into a `walking` first event of
          // `incoming`, instead of staying as two adjacent walking
          // entries across the boundary.
          const merged = batchEvents([...s.events, ...incoming]);
          let acc = { status: s.status, lastStats: s.lastStats };
          for (const e of incoming) {
            acc = foldEvent(acc, e);
          }
          return {
            events: merged.slice(-EVENT_CAP),
            status: acc.status,
            lastStats: acc.lastStats,
            moment:
              acc.status !== s.status
                ? momentAfterStatus(s.moment, acc.status)
                : s.moment,
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
          moment: "home",
        }),
      markCancelled: () =>
        set((s) =>
          s.status === "running" ? { status: "cancelled", moment: "home" } : s,
        ),
      patchOptions: (patch) =>
        set((s) => ({ options: { ...s.options, ...patch } })),
    }),
    {
      name: "app-state",
      storage: createJSONStorage(() => tauriStoreAdapter),
      partialize: (state) =>
        ({
          options: state.options,
          advancedOpen: state.advancedOpen,
          recentTargets: state.recentTargets,
        }) as Partial<AppState>,
      // Persisted state from v0.5 lacks the 8 new toggle fields; a naive
      // shallow merge would replace `defaultOptions` wholesale and leave
      // `dedupFiles` etc. undefined → all lossless defaults silently off.
      // Deep-merging `options` lets old installs inherit the new defaults
      // for fields they've never seen while preserving their existing
      // edits. New fields land via `defaultOptions`, persisted edits win
      // for fields the user has touched.
      merge: (persisted, current) => {
        const p = persisted as
          | {
              options?: Partial<PackOptions>;
              advancedOpen?: boolean;
              recentTargets?: unknown;
            }
          | undefined;
        const recents: RecentTarget[] = Array.isArray(p?.recentTargets)
          ? (p.recentTargets as unknown[])
              .filter(
                (r): r is RecentTarget =>
                  !!r &&
                  typeof r === "object" &&
                  ((r as RecentTarget).kind === "folder" ||
                    (r as RecentTarget).kind === "github") &&
                  typeof (r as RecentTarget).value === "string",
              )
              .slice(0, RECENTS_CAP)
          : current.recentTargets;
        return {
          ...current,
          options: { ...current.options, ...(p?.options ?? {}) },
          advancedOpen:
            typeof p?.advancedOpen === "boolean"
              ? p.advancedOpen
              : current.advancedOpen,
          recentTargets: recents,
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
      patchOptions: s.patchOptions,
    })),
  );
}

/** Subscribe to the live progress slice (status + result + jobId).
 * Deliberately excludes `events` — those land ~10×/second during a pack
 * and would re-render every subscriber per flush. Components that render
 * the event stream (progress log / progress bar) should use
 * `usePackEvents` so only they re-render per flush. */
export function usePackProgress() {
  return useApp(
    useShallow((s) => ({
      status: s.status,
      result: s.result,
      jobId: s.jobId,
    })),
  );
}

/** Subscribe to the raw progress-event stream. High-churn during a pack
 * (one update per 100ms flush) — keep subscribers small and leaf-level. */
export function usePackEvents(): ProgressEvent[] {
  return useApp((s) => s.events);
}

/** Subscribe to `lastStats` only. The Compression panel reads
 * `lastStats.transforms` to render per-row savings chips during and after
 * a pack — a dedicated selector keeps unrelated re-renders from triggering
 * a tree rebuild of the toggle list. */
export function useLastStats(): PackStats | null {
  return useApp((s) => s.lastStats);
}
