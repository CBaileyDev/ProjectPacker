import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { useShallow } from "zustand/react/shallow";
import type { PackOptions, PackResult, ProgressEvent } from "./api";
import { batchEvents } from "./events";
import { tauriStoreAdapter } from "./persist-adapter";

type PackingStatus = "idle" | "running" | "done" | "error";

interface AppState {
  jobId: string | null;
  status: PackingStatus;
  events: ProgressEvent[];
  result: PackResult | null;
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
  maxFileSizeKb: 1024,
  respectGitignore: true,
  customIgnorePatterns: [],
  protocolVersion: "grok-to-cc-v1",
  format: "xml",
  xmlSchema: "cxml",
};

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
      options: defaultOptions,
      setJob: (id) =>
        set({ jobId: id, status: "running", events: [], result: null }),
      pushEvent: (e) =>
        set((s) => ({
          events: [...s.events, e].slice(-EVENT_CAP),
          status: nextStatus(s.status, e),
        })),
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
          for (const e of incoming) {
            nextStat = nextStatus(nextStat, e);
          }
          return {
            events: merged.slice(-EVENT_CAP),
            status: nextStat,
          };
        }),
      setResult: (r) => set({ result: r }),
      reset: () =>
        set({ jobId: null, status: "idle", events: [], result: null }),
      setOptions: (o) => set({ options: o }),
      patchOptions: (patch) =>
        set((s) => ({ options: { ...s.options, ...patch } })),
    }),
    {
      name: "app-state",
      storage: createJSONStorage(() => tauriStoreAdapter),
      partialize: (state) => ({ options: state.options }) as Partial<AppState>,
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
