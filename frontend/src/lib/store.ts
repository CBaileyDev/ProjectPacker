import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { PackOptions, PackResult, ProgressEvent } from "./api";
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
  setResult: (r: PackResult) => void;
  reset: () => void;
  setOptions: (o: PackOptions) => void;
}

const defaultOptions: PackOptions = {
  target: { kind: "folder", value: "" },
  goal: "",
  includeGitHistory: false,
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
          events: [...s.events, e],
          status:
            e.kind === "done"
              ? "done"
              : e.kind === "error"
                ? "error"
                : s.status,
        })),
      setResult: (r) => set({ result: r }),
      reset: () =>
        set({ jobId: null, status: "idle", events: [], result: null }),
      setOptions: (o) => set({ options: o }),
    }),
    {
      name: "app-state",
      storage: createJSONStorage(() => tauriStoreAdapter),
      partialize: (state) => ({ options: state.options }),
    },
  ),
);
