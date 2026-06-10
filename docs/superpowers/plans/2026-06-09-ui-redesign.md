# UI Redesign — Four Moments, Graphite & Ember — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 5-tab sidebar UI with a single window that morphs through four moments (Home → Packing → Results → Bridge) in a new Graphite & Ember visual identity, with zero Rust/IPC changes.

**Architecture:** A `moment` state machine in the zustand store drives which full-screen view renders; pack status transitions move the moment automatically. GitHub/Settings become overlay sheets. Existing leaf components (BridgeTab, CompressionPanel, ProgressLog, StatsBar, etc.) are re-homed, not rewritten. Brand color moves to a `--primary` ember token; green/red are reserved for semantic status.

**Tech Stack:** React 18, zustand v5 (persist), framer-motion (LazyMotion `m.`), Tailwind v4 (`@theme inline` tokens), vitest + happy-dom, Tauri 2.

**Spec:** `docs/superpowers/specs/2026-06-09-ui-redesign-design.md`

**Branch:** work on `refactor/simplification-pass` (already checked out).

**Spec deviations (decided here, justified):**
1. `activePreset` is NOT persisted — it is derivable from `options` via `matchPreset()`. Persisting it would be redundant state.
2. Settings.tsx is NOT split — it already contains only app-level content (GitHub PAT + About); it becomes the Settings sheet body unchanged. Pack options already live in the packer tab and move to Home's Advanced panel.
3. Recents are stored frontend-side (`recentTargets` in the zustand store, recorded on pack start, cap 5) — the Rust `Settings.recents` field is never written by anything today, and wiring it would be backend scope.

**Verification commands (used throughout):**
- `cd frontend && pnpm typecheck` — expect exit 0
- `cd frontend && pnpm vitest run <file>` — expect listed tests pass
- `cd frontend && pnpm test` — full suite, expect all pass
- `cd frontend && pnpm lint` — biome; expect ONLY the 8 pre-existing errors (use-drag-drop ×6, ProgressLog ×1, GithubConnector ×1), nothing new

## File structure

```
frontend/src/
  lib/
    model-windows.ts          NEW  static context-window table + fitsIn()
    model-windows.test.ts     NEW
    presets.ts                NEW  Balanced/Minimal/Everything bundles
    presets.test.ts           NEW
    pack-meta.ts              NEW  FORMAT_LABELS, SAVE_FILENAMES, isValidTargetValue, …
    store.ts                  MOD  moment machine, sheets, advancedOpen, recentTargets
    store.test.ts             NEW
    persist-adapter.ts        MOD  sanitize whitelist for new persisted keys
    persist-adapter.test.ts   MOD  new cases
  components/
    shell/TitleBar.tsx        NEW
    shell/Sheet.tsx           NEW
    home/SectionTitle.tsx     NEW  (moved from Pack.tsx)
    home/TargetSection.tsx    NEW  (moved from Pack.tsx)
    home/GoalSection.tsx      NEW  (moved from Pack.tsx)
    home/OnboardingCard.tsx   NEW  (moved from Pack.tsx)
    results/FitsCard.tsx      NEW  (replaces AiContextTable)
  routes/
    Home.tsx                  NEW
    Packing.tsx               NEW
    Results.tsx               NEW
    Bridge.tsx                NEW
    Pack.tsx                  DELETED (Task 12)
  components/pack/AiContextTable.tsx  DELETED (Task 12)
  App.tsx                     MOD  shell swap
  styles/globals.css          MOD  Graphite & Ember palette
```

---

### Task 1: Graphite & Ember palette tokens

**Files:**
- Modify: `frontend/src/styles/globals.css`

The old UI uses hardcoded `emerald-*` utilities so this token change barely
shows yet — that's intentional; new components (Tasks 7–11) consume tokens
and render ember immediately, and Task 13 sweeps the survivors.

- [ ] **Step 1: Replace the `:root` custom-property block**

In `frontend/src/styles/globals.css`, replace the existing `--gradient-bg`,
`--glow-emerald`, and `--glow-border` declarations inside `:root` with:

```css
  --gradient-bg:
    radial-gradient(ellipse 80% 60% at 50% -10%, rgba(245, 158, 66, 0.07) 0%, transparent 60%),
    radial-gradient(ellipse 60% 40% at 80% 90%, rgba(99, 102, 241, 0.04) 0%, transparent 50%);
  --surface-raised: oklch(0.985 0.003 85);
  --surface-sunken: oklch(0.96 0.004 85);
  --glow-accent: 0 0 20px rgba(245, 158, 66, 0.18);
  --glow-border: 0 0 0 1px rgba(255, 255, 255, 0.06);
```

Then in the same `:root` block update these light-theme tokens (leave any
token not listed here unchanged):

```css
  --background: oklch(0.975 0.005 85);
  --foreground: oklch(0.17 0.01 75);
  --primary: oklch(0.72 0.15 65);
  --primary-foreground: oklch(0.18 0.05 70);
  --ring: oklch(0.72 0.15 65 / 0.4);
```

- [ ] **Step 2: Update the `.dark` block**

In the `.dark` selector block, set (leave unlisted tokens unchanged):

```css
  --background: oklch(0.16 0.005 75);
  --foreground: oklch(0.93 0.005 85);
  --card: oklch(0.19 0.006 75);
  --card-foreground: oklch(0.93 0.005 85);
  --primary: oklch(0.78 0.14 70);
  --primary-foreground: oklch(0.2 0.04 70);
  --ring: oklch(0.78 0.14 70 / 0.4);
```

- [ ] **Step 3: Grep for the old glow variable**

Run: `grep -rn "glow-emerald" frontend/src/`
For every hit, replace `--glow-emerald` / `glow-emerald` with `--glow-accent`
/ `glow-accent`. Expected: zero hits afterwards.

- [ ] **Step 4: Verify**

Run: `cd frontend && pnpm typecheck && pnpm test`
Expected: typecheck exit 0; all 24 tests pass.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/styles/globals.css $(grep -rl "glow-accent" frontend/src/ || true)
git commit -m "feat(ui): Graphite & Ember palette tokens"
```

---

### Task 2: `lib/model-windows.ts`

**Files:**
- Create: `frontend/src/lib/model-windows.ts`
- Test: `frontend/src/lib/model-windows.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import { fitsIn, MODEL_WINDOWS } from "./model-windows";

describe("model-windows", () => {
  it("covers all seven TokensPerModel keys exactly once", () => {
    const keys = MODEL_WINDOWS.map((m) => m.key).sort();
    expect(keys).toEqual(
      [
        "claude",
        "deepSeek",
        "geminiApprox",
        "gpt4o",
        "llama3",
        "mistral",
        "qwen2_5",
      ].sort(),
    );
  });

  it("classifies fits / tight / over at the boundaries", () => {
    expect(fitsIn(75_000, 100_000)).toBe("fits"); // exactly 75% is still fits
    expect(fitsIn(75_001, 100_000)).toBe("tight"); // >75%
    expect(fitsIn(100_000, 100_000)).toBe("tight"); // exactly the window
    expect(fitsIn(100_001, 100_000)).toBe("over"); // over
    expect(fitsIn(0, 100_000)).toBe("fits");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd frontend && pnpm vitest run src/lib/model-windows.test.ts`
Expected: FAIL — "Failed to resolve import ./model-windows"

- [ ] **Step 3: Write the implementation**

```typescript
import type { TokensPerModel } from "../bindings";

/** Advertised context windows (tokens), deliberately conservative round
 * numbers — this powers a fits/tight/over hint, not billing. Update when
 * vendors move the goalposts. */
export interface ModelWindow {
  key: keyof TokensPerModel;
  label: string;
  window: number;
}

export const MODEL_WINDOWS: readonly ModelWindow[] = [
  { key: "claude", label: "Claude", window: 200_000 },
  { key: "geminiApprox", label: "Gemini (approx)", window: 1_000_000 },
  { key: "gpt4o", label: "GPT-4o", window: 128_000 },
  { key: "llama3", label: "Llama 3", window: 128_000 },
  { key: "qwen2_5", label: "Qwen 2.5", window: 128_000 },
  { key: "deepSeek", label: "DeepSeek", window: 128_000 },
  { key: "mistral", label: "Mistral", window: 128_000 },
];

export type Fit = "fits" | "tight" | "over";

/** Strictly-over-the-window is "over"; above 75% utilisation is "tight". */
export function fitsIn(tokens: number, window: number): Fit {
  if (tokens > window) return "over";
  if (tokens > window * 0.75) return "tight";
  return "fits";
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd frontend && pnpm vitest run src/lib/model-windows.test.ts`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/model-windows.ts frontend/src/lib/model-windows.test.ts
git commit -m "feat(ui): model context-window table + fitsIn helper"
```

---

### Task 3: `lib/presets.ts`

**Files:**
- Create: `frontend/src/lib/presets.ts`
- Test: `frontend/src/lib/presets.test.ts`

- [ ] **Step 1: Write the failing test**

```typescript
import { describe, expect, it } from "vitest";
import type { PackOptions } from "../bindings";
import { applyPreset, matchPreset, PRESETS } from "./presets";

const base: PackOptions = {
  target: { kind: "folder", value: "/tmp/x" },
  goal: "",
  countTokens: true,
  tokenizerModel: "gpt-4o-mini",
  secretScan: true,
  compress: false,
  removeComments: false,
  dedupFiles: true,
  trimTrailingWs: true,
  collapseBlankLines: true,
  normalizeLineEndings: true,
  collapseLockfiles: false,
  collapseMinified: false,
  markGenerated: false,
  elideTypeOnlyExports: false,
  maxFileSizeKb: 1024,
  respectGitignore: true,
  customIgnorePatterns: [],
  protocolVersion: "plan-exec-v1",
  format: "xml",
  xmlSchema: "cxml",
};

describe("presets", () => {
  it("defines balanced, minimal, everything", () => {
    expect(PRESETS.map((p) => p.id)).toEqual([
      "balanced",
      "minimal",
      "everything",
    ]);
  });

  it("round-trips: applying a preset makes matchPreset return its id", () => {
    for (const p of PRESETS) {
      expect(matchPreset(applyPreset(base, p))).toBe(p.id);
    }
  });

  it("apply only touches patch fields", () => {
    const minimal = PRESETS.find((p) => p.id === "minimal");
    if (!minimal) throw new Error("minimal preset missing");
    const next = applyPreset(base, minimal);
    expect(next.target).toEqual(base.target);
    expect(next.goal).toBe(base.goal);
    expect(next.format).toBe(base.format);
    expect(next.compress).toBe(true);
    expect(next.maxFileSizeKb).toBe(512);
  });

  it("returns null when options match no preset", () => {
    expect(matchPreset({ ...base, maxFileSizeKb: 777 })).toBeNull();
  });

  it("base defaults match the balanced preset", () => {
    expect(matchPreset(base)).toBe("balanced");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd frontend && pnpm vitest run src/lib/presets.test.ts`
Expected: FAIL — "Failed to resolve import ./presets"

- [ ] **Step 3: Write the implementation**

```typescript
import type { PackOptions } from "../bindings";

/** A preset is a named bundle over existing PackOptions fields. Matching
 * compares ONLY the patch keys, so target/goal/format never disqualify a
 * preset. The chip row derives its highlighted state via matchPreset —
 * the active preset is never stored. */
export interface Preset {
  id: "balanced" | "minimal" | "everything";
  label: string;
  description: string;
  patch: Partial<PackOptions>;
}

export const PRESETS: readonly Preset[] = [
  {
    id: "balanced",
    label: "Balanced",
    description: "Lossless cleanup, full-fidelity code",
    patch: {
      compress: false,
      removeComments: false,
      dedupFiles: true,
      trimTrailingWs: true,
      collapseBlankLines: true,
      normalizeLineEndings: true,
      collapseLockfiles: false,
      collapseMinified: false,
      markGenerated: false,
      elideTypeOnlyExports: false,
      maxFileSizeKb: 1024,
    },
  },
  {
    id: "minimal",
    label: "Minimal tokens",
    description: "Every safe compressor on, small files only",
    patch: {
      compress: true,
      removeComments: true,
      dedupFiles: true,
      trimTrailingWs: true,
      collapseBlankLines: true,
      normalizeLineEndings: true,
      collapseLockfiles: true,
      collapseMinified: true,
      markGenerated: true,
      elideTypeOnlyExports: true,
      maxFileSizeKb: 512,
    },
  },
  {
    id: "everything",
    label: "Everything",
    description: "No transforms — raw files, max size cap",
    patch: {
      compress: false,
      removeComments: false,
      dedupFiles: false,
      trimTrailingWs: false,
      collapseBlankLines: false,
      normalizeLineEndings: false,
      collapseLockfiles: false,
      collapseMinified: false,
      markGenerated: false,
      elideTypeOnlyExports: false,
      maxFileSizeKb: 102_400,
    },
  },
];

export function applyPreset(options: PackOptions, preset: Preset): PackOptions {
  return { ...options, ...preset.patch };
}

export function matchPreset(options: PackOptions): Preset["id"] | null {
  for (const p of PRESETS) {
    const matches = Object.entries(p.patch).every(
      ([k, v]) => options[k as keyof PackOptions] === v,
    );
    if (matches) return p.id;
  }
  return null;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd frontend && pnpm vitest run src/lib/presets.test.ts`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/presets.ts frontend/src/lib/presets.test.ts
git commit -m "feat(ui): pack option presets (balanced/minimal/everything)"
```

---

### Task 4: Store — moment machine, sheets, advanced flag, recents

**Files:**
- Modify: `frontend/src/lib/store.ts`
- Test: `frontend/src/lib/store.test.ts` (new)

- [ ] **Step 1: Write the failing tests**

Create `frontend/src/lib/store.test.ts`:

```typescript
import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the Tauri plugin-store boundary exactly like persist-adapter.test.ts.
const backing = new Map<string, unknown>();
vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    async get(key: string) {
      return backing.get(key);
    }
    async set(key: string, value: unknown) {
      backing.set(key, value);
    }
    async save() {}
    async delete(key: string) {
      backing.delete(key);
    }
  },
}));

import type { ProgressEvent } from "./api";
import { useApp } from "./store";

const doneEvent: ProgressEvent = {
  kind: "done",
  stats: {
    filesTotal: 1,
    filesIncluded: 1,
    filesSkipped: 0,
    bytesTotal: 10,
    tokensTotal: 5,
    tokensPerModel: null,
    secretsFound: 0,
    durationMs: 1,
    walkMs: 0,
    processMs: 0,
    secretScanMs: null,
    tokenizeMs: null,
    emitMs: 1,
    transforms: [],
    transformPhaseMs: 0,
  },
};

const errorEvent: ProgressEvent = {
  kind: "error",
  message: "boom",
  fatal: true,
};

function resetStore() {
  useApp.setState({
    jobId: null,
    status: "idle",
    events: [],
    result: null,
    lastStats: null,
    moment: "home",
    activeSheet: null,
    recentTargets: [],
  });
}

describe("moment state machine", () => {
  beforeEach(resetStore);

  it("boots into home", () => {
    expect(useApp.getState().moment).toBe("home");
  });

  it("setJob moves to packing", () => {
    useApp.getState().setJob("job-1");
    expect(useApp.getState().moment).toBe("packing");
    expect(useApp.getState().status).toBe("running");
  });

  it("done event moves to results", () => {
    useApp.getState().setJob("job-1");
    useApp.getState().pushEvent(doneEvent);
    expect(useApp.getState().moment).toBe("results");
    expect(useApp.getState().status).toBe("done");
  });

  it("batched done event moves to results", () => {
    useApp.getState().setJob("job-1");
    useApp.getState().pushEventsBatched([doneEvent]);
    expect(useApp.getState().moment).toBe("results");
  });

  it("error event returns home", () => {
    useApp.getState().setJob("job-1");
    useApp.getState().pushEvent(errorEvent);
    expect(useApp.getState().moment).toBe("home");
    expect(useApp.getState().status).toBe("error");
  });

  it("cancel returns home", () => {
    useApp.getState().setJob("job-1");
    useApp.getState().markCancelled();
    expect(useApp.getState().moment).toBe("home");
    expect(useApp.getState().status).toBe("cancelled");
  });

  it("reset returns home", () => {
    useApp.getState().setJob("job-1");
    useApp.getState().pushEvent(doneEvent);
    useApp.getState().reset();
    expect(useApp.getState().moment).toBe("home");
  });

  it("setMoment navigates results ⇄ bridge", () => {
    useApp.getState().setMoment("bridge");
    expect(useApp.getState().moment).toBe("bridge");
    useApp.getState().setMoment("results");
    expect(useApp.getState().moment).toBe("results");
  });

  it("manages overlay sheets", () => {
    useApp.getState().setSheet("github");
    expect(useApp.getState().activeSheet).toBe("github");
    useApp.getState().setSheet(null);
    expect(useApp.getState().activeSheet).toBeNull();
  });
});

describe("recent targets", () => {
  beforeEach(resetStore);

  it("records the target on pack start, most recent first, deduped, cap 5", () => {
    const s = useApp.getState();
    for (let i = 1; i <= 6; i++) {
      s.patchOptions({ target: { kind: "folder", value: `/repo-${i}` } });
      useApp.getState().setJob(`job-${i}`);
    }
    // Re-pack repo-3: moves to front, no duplicate.
    useApp.getState().patchOptions({ target: { kind: "folder", value: "/repo-3" } });
    useApp.getState().setJob("job-7");

    const recents = useApp.getState().recentTargets;
    expect(recents.length).toBe(5);
    expect(recents[0]).toEqual({ kind: "folder", value: "/repo-3" });
    expect(
      recents.filter((r) => r.value === "/repo-3").length,
    ).toBe(1);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd frontend && pnpm vitest run src/lib/store.test.ts`
Expected: FAIL — `moment`/`setMoment`/`recentTargets` do not exist.

- [ ] **Step 3: Implement the store changes**

In `frontend/src/lib/store.ts`:

3a. Add below the `PackingStatus` type:

```typescript
export type Moment = "home" | "packing" | "results" | "bridge";
export type SheetId = "github" | "settings";

export interface RecentTarget {
  kind: "folder" | "github";
  value: string;
}

/** Moment transition applied when the pack status CHANGES. `done` lands on
 * Results; a fatal error returns Home (the error surfaces as a banner).
 * All other statuses leave the moment alone. */
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
```

3b. Extend `interface AppState` with:

```typescript
  /** Which full-screen view is showing. Driven by user navigation and by
   * pack-status transitions (setJob → packing, done → results,
   * error/cancel → home). Never persisted — the app always boots Home. */
  moment: Moment;
  /** Overlay sheet (GitHub / Settings) — orthogonal to `moment`. */
  activeSheet: SheetId | null;
  /** Whether Home's Advanced options panel is expanded. Persisted. */
  advancedOpen: boolean;
  /** Last 5 pack targets, most recent first. Persisted. */
  recentTargets: RecentTarget[];
  setMoment: (m: Moment) => void;
  setSheet: (s: SheetId | null) => void;
  setAdvancedOpen: (v: boolean) => void;
```

3c. In the `create` initializer add the initial values and actions:

```typescript
      moment: "home",
      activeSheet: null,
      advancedOpen: false,
      recentTargets: [],
      setMoment: (m) => set({ moment: m }),
      setSheet: (sheet) => set({ activeSheet: sheet }),
      setAdvancedOpen: (v) => set({ advancedOpen: v }),
```

3d. Update the existing actions:

```typescript
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
```

In `pushEvent`, change the returned object to also derive the moment (only
when status changed):

```typescript
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
```

In `pushEventsBatched`, change the returned object the same way:

```typescript
          return {
            events: merged.slice(-EVENT_CAP),
            status: acc.status,
            lastStats: acc.lastStats,
            moment:
              acc.status !== s.status
                ? momentAfterStatus(s.moment, acc.status)
                : s.moment,
          };
```

Update `reset` and `markCancelled`:

```typescript
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
          s.status === "running"
            ? { status: "cancelled", moment: "home" }
            : s,
        ),
```

3e. Update persistence — `partialize` and `merge`:

```typescript
      partialize: (state) =>
        ({
          options: state.options,
          advancedOpen: state.advancedOpen,
          recentTargets: state.recentTargets,
        }) as Partial<AppState>,
```

```typescript
      merge: (persisted, current) => {
        const p = persisted as
          | {
              options?: Partial<PackOptions>;
              advancedOpen?: boolean;
              recentTargets?: RecentTarget[];
            }
          | undefined;
        return {
          ...current,
          options: { ...current.options, ...(p?.options ?? {}) },
          advancedOpen: p?.advancedOpen ?? false,
          recentTargets: p?.recentTargets ?? [],
        };
      },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd frontend && pnpm vitest run src/lib/store.test.ts`
Expected: 10 passed.

- [ ] **Step 5: Full suite + typecheck**

Run: `cd frontend && pnpm typecheck && pnpm test`
Expected: exit 0, all tests pass (now 34+).

- [ ] **Step 6: Commit**

```bash
git add frontend/src/lib/store.ts frontend/src/lib/store.test.ts
git commit -m "feat(ui): moment state machine, sheets, recents in store"
```

---

### Task 5: Sanitize whitelist for the new persisted keys

**Files:**
- Modify: `frontend/src/lib/persist-adapter.ts`
- Modify: `frontend/src/lib/persist-adapter.test.ts`

- [ ] **Step 1: Add failing tests** (append inside the existing `describe`)

```typescript
  it("coerces bad advancedOpen / recentTargets on cold read", async () => {
    backing.set(
      "app-state",
      JSON.stringify({
        state: {
          options: { protocolVersion: PROTOCOL_VERSION, maxFileSizeKb: 1024, format: "xml", goal: "" },
          advancedOpen: "yes",
          recentTargets: [
            { kind: "folder", value: "/ok" },
            { kind: "evil", value: "/bad" },
            { kind: "github", value: 42 },
            { kind: "github", value: "https://github.com/a/b" },
            { kind: "folder", value: "/2" },
            { kind: "folder", value: "/3" },
            { kind: "folder", value: "/4" },
            { kind: "folder", value: "/5" },
          ],
        },
        version: 0,
      }),
    );
    const raw = await tauriStoreAdapter.getItem("app-state");
    const state = JSON.parse(raw as string).state;
    expect(state.advancedOpen).toBe(false);
    // invalid entries dropped, cap 5
    expect(state.recentTargets).toEqual([
      { kind: "folder", value: "/ok" },
      { kind: "github", value: "https://github.com/a/b" },
      { kind: "folder", value: "/2" },
      { kind: "folder", value: "/3" },
      { kind: "folder", value: "/4" },
    ]);
  });
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd frontend && pnpm vitest run src/lib/persist-adapter.test.ts`
Expected: FAIL — `advancedOpen` comes back as `"yes"`.

- [ ] **Step 3: Extend `sanitize()`**

In `frontend/src/lib/persist-adapter.ts`, inside `sanitize()` after the
`if (!options) return raw;` guard, the function only touched `options`.
Restructure minimally: change the guard so state-level keys are sanitized
even when `options` is present (keep the existing options checks), and add
before `return JSON.stringify(parsed);`:

```typescript
  // advancedOpen — boolean or reset.
  if (typeof state.advancedOpen !== "boolean") {
    state.advancedOpen = false;
  }

  // recentTargets — array of {kind: folder|github, value: string}, cap 5.
  const rawRecents = Array.isArray(state.recentTargets)
    ? state.recentTargets
    : [];
  state.recentTargets = rawRecents
    .filter(
      (r): r is { kind: string; value: string } =>
        !!r &&
        typeof r === "object" &&
        ((r as { kind?: unknown }).kind === "folder" ||
          (r as { kind?: unknown }).kind === "github") &&
        typeof (r as { value?: unknown }).value === "string",
    )
    .slice(0, 5);
```

Note: `state` is typed `Record<string, unknown> | undefined`; add a guard
`if (!state) return raw;` above the options lookup if not already present,
and update the doc comment's "Validations" list to mention the two new keys.

- [ ] **Step 4: Run to verify green**

Run: `cd frontend && pnpm vitest run src/lib/persist-adapter.test.ts`
Expected: all pass (3 tests).

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/persist-adapter.ts frontend/src/lib/persist-adapter.test.ts
git commit -m "feat(ui): sanitize advancedOpen + recentTargets on rehydration"
```

---

### Task 6: Extract shared pack metadata + home sections (Pack.tsx keeps working)

**Files:**
- Create: `frontend/src/lib/pack-meta.ts`
- Create: `frontend/src/components/home/SectionTitle.tsx`
- Create: `frontend/src/components/home/TargetSection.tsx`
- Create: `frontend/src/components/home/GoalSection.tsx`
- Create: `frontend/src/components/home/OnboardingCard.tsx`
- Modify: `frontend/src/routes/Pack.tsx` (imports only)

This is a pure move so Tasks 8–11 can import these pieces while the old
Pack.tsx still runs the app.

- [ ] **Step 1: Create `frontend/src/lib/pack-meta.ts`**

Move these verbatim from `routes/Pack.tsx` (lines 58–79 and 156–158) and
export them:

```typescript
import type { PackFormat } from "../bindings";

export const FORMAT_LABELS: Record<PackFormat, string> = {
  xml: "XML  (planner / executor)",
  markdown: "Markdown",
  plainText: "Plain Text",
};

export const COPY_BUTTON_LABELS: Record<PackFormat, string> = {
  xml: "Copy Pack XML",
  markdown: "Copy Pack Markdown",
  plainText: "Copy Plain Text",
};

export const SAVE_FILENAMES: Record<PackFormat, string> = {
  xml: "pack.xml",
  markdown: "pack.md",
  plainText: "pack.txt",
};

export const GITHUB_URL_PATTERN =
  /^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+(\.git)?\/?$/;

export const MAX_FILE_SIZE_KB = 102_400;

export function isValidTargetValue(
  kind: "folder" | "github",
  value: string,
): boolean {
  return kind === "folder" ? value.length > 0 : GITHUB_URL_PATTERN.test(value);
}
```

- [ ] **Step 2: Create the four `components/home/` files**

Move each component verbatim from `routes/Pack.tsx` into its own file with
the imports it needs. `SectionTitle.tsx` exports `SectionTitle` (Pack.tsx
lines 160–166). `TargetSection.tsx` exports `TargetSection` (lines 752–895;
import `isValidTargetValue` from `../../lib/pack-meta`, icons from
`../pack/icons`, motion from `../../lib/motion`, `useApp` from
`../../lib/store`, `open` from `@tauri-apps/plugin-dialog`, `SectionTitle`
from `./SectionTitle`, `AnimatePresence` from `framer-motion`, `* as m`
from `framer-motion/m`). `GoalSection.tsx` exports `GoalSection`
(lines 897–912). `OnboardingCard.tsx` exports `ONBOARDING_STEPS` (not
exported — keep private) and `OnboardingCard` (lines 919–982).

The component bodies are copied byte-for-byte — no behavior or styling
changes in this task.

- [ ] **Step 3: Update `routes/Pack.tsx`**

Delete the moved code from Pack.tsx and replace with imports:

```typescript
import { GoalSection } from "../components/home/GoalSection";
import { OnboardingCard } from "../components/home/OnboardingCard";
import { SectionTitle } from "../components/home/SectionTitle";
import { TargetSection } from "../components/home/TargetSection";
import {
  COPY_BUTTON_LABELS,
  FORMAT_LABELS,
  isValidTargetValue,
  MAX_FILE_SIZE_KB,
  SAVE_FILENAMES,
} from "../lib/pack-meta";
```

Remove the now-unused imports from Pack.tsx (`open` from plugin-dialog,
`FolderIcon`/`GithubIcon` if no longer referenced — check with typecheck/lint).

- [ ] **Step 4: Verify**

Run: `cd frontend && pnpm typecheck && pnpm test && pnpm lint`
Expected: typecheck 0; all tests pass; lint shows ONLY the 8 pre-existing errors.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/pack-meta.ts frontend/src/components/home/ frontend/src/routes/Pack.tsx
git commit -m "refactor(ui): extract pack metadata + home sections from Pack.tsx"
```

---

### Task 7: Shell primitives — `Sheet` and `TitleBar`

**Files:**
- Create: `frontend/src/components/shell/Sheet.tsx`
- Create: `frontend/src/components/shell/TitleBar.tsx`

Not mounted yet (Task 12 mounts them) — this task only has to typecheck.

- [ ] **Step 1: Create `Sheet.tsx`**

```tsx
import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { type ReactNode, useEffect, useRef } from "react";
import { prefersReducedMotion } from "../../lib/motion";
import { XIcon } from "../pack/icons";

interface SheetProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/** Right-side overlay panel for GitHub / Settings. Scrim click and the ✕
 * button close it; Esc is handled globally in App. Focus moves into the
 * panel on open and back to the previously-focused element on close. */
export function Sheet({ open, title, onClose, children }: SheetProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (open) {
      restoreRef.current = document.activeElement as HTMLElement | null;
      panelRef.current?.focus();
    } else {
      restoreRef.current?.focus?.();
    }
  }, [open]);

  return (
    <AnimatePresence>
      {open && (
        <m.div
          className="fixed inset-0 z-modal flex justify-end"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: prefersReducedMotion ? 0 : 0.15 }}
        >
          {/* biome-ignore lint/a11y/useKeyWithClickEvents: Esc is handled globally */}
          <div
            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
            onClick={onClose}
            aria-hidden="true"
          />
          <m.div
            ref={panelRef}
            role="dialog"
            aria-modal="true"
            aria-label={title}
            tabIndex={-1}
            className="relative flex h-full w-full max-w-xl flex-col overflow-y-auto border-l border-white/10 bg-background p-6 shadow-2xl outline-none"
            initial={
              prefersReducedMotion ? { opacity: 0 } : { x: 48, opacity: 0 }
            }
            animate={{ x: 0, opacity: 1 }}
            exit={prefersReducedMotion ? { opacity: 0 } : { x: 48, opacity: 0 }}
            transition={{
              duration: prefersReducedMotion ? 0 : 0.22,
              ease: [0.22, 1, 0.36, 1],
            }}
          >
            <div className="mb-5 flex items-center justify-between">
              <h2 className="text-lg font-bold tracking-tight">{title}</h2>
              <button
                type="button"
                onClick={onClose}
                aria-label={`Close ${title}`}
                className="rounded-lg p-2 text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
              >
                <XIcon size={16} />
              </button>
            </div>
            {children}
          </m.div>
        </m.div>
      )}
    </AnimatePresence>
  );
}
```

- [ ] **Step 2: Create `TitleBar.tsx`**

```tsx
import { useApp, usePackProgress } from "../../lib/store";
import { useGithubToken } from "../../lib/use-github-token";
import { isMacPlatform } from "../../lib/use-keyboard-shortcuts";
import { GithubIcon, PackageIcon, SettingsIcon } from "../pack/icons";

/** Persistent top strip: wordmark (click → Home), ⌘K hint, GitHub status,
 * settings gear. The only chrome that survives across moments. */
export function TitleBar() {
  const moment = useApp((s) => s.moment);
  const setMoment = useApp((s) => s.setMoment);
  const setSheet = useApp((s) => s.setSheet);
  const { status } = usePackProgress();
  const { hasToken, ready } = useGithubToken();

  return (
    <header className="flex items-center gap-3 px-5 py-3">
      <button
        type="button"
        onClick={() => {
          if (status !== "running") setMoment("home");
        }}
        className="flex items-center gap-2.5 rounded-lg px-1.5 py-1 transition-colors hover:bg-white/5"
        aria-label="Go to Home"
      >
        <span className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary/15 ring-1 ring-primary/25">
          <PackageIcon size={15} className="text-primary" />
        </span>
        <span className="text-sm font-bold tracking-tight">ProjectPacker</span>
      </button>

      <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-primary/80">
        plan-exec-v1
      </span>

      <div className="ml-auto flex items-center gap-1.5">
        <kbd className="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 font-sans text-[10px] text-zinc-500">
          {isMacPlatform() ? "⌘K" : "Ctrl K"}
        </kbd>
        <button
          type="button"
          onClick={() => setSheet("github")}
          aria-label="GitHub"
          title={ready && hasToken ? "GitHub — connected" : "GitHub"}
          className="relative rounded-lg p-2 text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
        >
          <GithubIcon size={16} />
          {ready && hasToken && (
            <span
              aria-hidden="true"
              className="absolute right-1 top-1 h-1.5 w-1.5 rounded-full bg-primary"
            />
          )}
        </button>
        <button
          type="button"
          onClick={() => setSheet("settings")}
          aria-label="Settings"
          className="rounded-lg p-2 text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
        >
          <SettingsIcon size={16} />
        </button>
      </div>
      {/* moment is read so the wordmark button can stay subtle on Home */}
      <span className="sr-only">{moment}</span>
    </header>
  );
}
```

- [ ] **Step 3: Verify + commit**

Run: `cd frontend && pnpm typecheck`
Expected: exit 0.

```bash
git add frontend/src/components/shell/
git commit -m "feat(ui): Sheet and TitleBar shell primitives"
```

---

### Task 8: `routes/Home.tsx`

**Files:**
- Create: `frontend/src/routes/Home.tsx`

Home composes: headline, TargetSection, GoalSection, preset chips, recents,
Advanced disclosure (toggles + CompressionPanel + format/max-size), the
Pack button row, error/cancelled banners, onboarding card.

- [ ] **Step 1: Create the component**

```tsx
import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { useMemo } from "react";
import type { PackFormat } from "../bindings";
import { GoalSection } from "../components/home/GoalSection";
import { OnboardingCard } from "../components/home/OnboardingCard";
import { TargetSection } from "../components/home/TargetSection";
import { CompressionPanel } from "../components/pack/CompressionPanel";
import {
  AlertIcon,
  ChevronDownIcon,
  FolderIcon,
  GithubIcon,
  LoaderIcon,
  PlayIcon,
  XIcon,
} from "../components/pack/icons";
import { Toggle } from "../components/pack/Toggle";
import {
  FORMAT_LABELS,
  isValidTargetValue,
  MAX_FILE_SIZE_KB,
} from "../lib/pack-meta";
import { fadeUp, prefersReducedMotion, springButton } from "../lib/motion";
import { applyPreset, matchPreset, PRESETS } from "../lib/presets";
import { ProgressLog } from "../components/pack/ProgressLog";
import {
  useApp,
  usePackEvents,
  usePackOptions,
  usePackProgress,
} from "../lib/store";
import {
  isMacPlatform,
  useKeyboardShortcuts,
} from "../lib/use-keyboard-shortcuts";
import { usePackJob } from "../lib/use-pack-job";

/** Derive a short display label for a recent target. */
function recentLabel(value: string, kind: "folder" | "github"): string {
  if (kind === "github") {
    return value.replace(/^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)/, "").replace(/(\.git)?\/?$/, "");
  }
  const parts = value.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? value;
}

export default function Home() {
  const { options, patchOptions } = usePackOptions();
  const { status, result } = usePackProgress();
  const setSheet = useApp((s) => s.setSheet);
  const advancedOpen = useApp((s) => s.advancedOpen);
  const setAdvancedOpen = useApp((s) => s.setAdvancedOpen);
  const recentTargets = useApp((s) => s.recentTargets);
  const events = usePackEvents();
  const { run: runPack, isRunning, errorMsg, dismissError } = usePackJob();

  const isValidTarget = useMemo(
    () => isValidTargetValue(options.target.kind, options.target.value),
    [options.target.kind, options.target.value],
  );
  const activePreset = useMemo(() => matchPreset(options), [options]);
  const isCancelled = status === "cancelled";
  const showOnboarding =
    status === "idle" && !result && options.target.value.length === 0;

  useKeyboardShortcuts({
    "mod+enter": () => {
      if (!isRunning && isValidTarget) runPack();
    },
  });

  return (
    <m.div
      className="mx-auto w-full max-w-2xl space-y-7 px-6 pb-16 pt-10"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="text-center">
        <h1 className="text-3xl font-bold tracking-tight">
          What are we packing?
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          Turn any repo into a planner-ready context pack.
        </p>
      </div>

      <TargetSection onBrowseGithub={() => setSheet("github")} />
      <GoalSection />

      {/* Preset chips */}
      <section className="space-y-3">
        <div className="flex flex-wrap items-center justify-center gap-2">
          {PRESETS.map((p) => {
            const on = activePreset === p.id;
            return (
              <button
                key={p.id}
                type="button"
                title={p.description}
                aria-pressed={on}
                onClick={() => patchOptions(applyPreset(options, p))}
                className={`rounded-full border px-4 py-1.5 text-xs font-medium transition-colors ${
                  on
                    ? "border-primary/50 bg-primary/10 text-primary"
                    : "border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"
                }`}
              >
                {p.label}
              </button>
            );
          })}
          <button
            type="button"
            aria-pressed={advancedOpen}
            onClick={() => setAdvancedOpen(!advancedOpen)}
            className={`flex items-center gap-1 rounded-full border px-4 py-1.5 text-xs font-medium transition-colors ${
              advancedOpen || activePreset === null
                ? "border-primary/50 bg-primary/10 text-primary"
                : "border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"
            }`}
          >
            Custom
            <ChevronDownIcon
              size={12}
              className={`transition-transform ${advancedOpen ? "rotate-180" : ""}`}
            />
          </button>
        </div>
      </section>

      {/* Advanced options */}
      <AnimatePresence initial={false}>
        {advancedOpen && (
          <m.section
            key="advanced"
            className="overflow-hidden"
            initial={prefersReducedMotion ? false : { height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={
              prefersReducedMotion
                ? { opacity: 0 }
                : { height: 0, opacity: 0 }
            }
            transition={{ duration: prefersReducedMotion ? 0 : 0.25 }}
          >
            <div className="space-y-4 rounded-xl border border-white/10 bg-white/[0.03] p-5">
              <div className="grid gap-3 md:grid-cols-2">
                <Toggle
                  label="Respect .gitignore"
                  checked={options.respectGitignore}
                  onChange={(v) => patchOptions({ respectGitignore: v })}
                />
                <Toggle
                  label="Scan for secrets"
                  checked={options.secretScan}
                  onChange={(v) => patchOptions({ secretScan: v })}
                />
                <Toggle
                  label="Count tokens"
                  hint="7 model tokenizers"
                  checked={options.countTokens}
                  onChange={(v) => patchOptions({ countTokens: v })}
                />
              </div>

              <CompressionPanel />

              <div className="grid gap-4 rounded-xl border border-white/10 bg-black/20 p-4 md:grid-cols-2">
                <label className="flex flex-wrap items-center gap-2">
                  <span className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                    Output Format
                  </span>
                  <select
                    className="rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 text-sm text-zinc-100 focus:border-primary/50 focus:outline-none"
                    value={options.format}
                    onChange={(e) =>
                      patchOptions({ format: e.target.value as PackFormat })
                    }
                  >
                    {(Object.keys(FORMAT_LABELS) as PackFormat[]).map((f) => (
                      <option key={f} value={f}>
                        {FORMAT_LABELS[f]}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-wrap items-center gap-2">
                  <span className="text-xs font-semibold uppercase tracking-wider text-zinc-500">
                    Max File Size
                  </span>
                  <input
                    type="number"
                    min={1}
                    max={MAX_FILE_SIZE_KB}
                    className="w-20 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1.5 text-sm text-zinc-100 focus:border-primary/50 focus:outline-none"
                    value={options.maxFileSizeKb}
                    onChange={(e) => {
                      const parsed = Number(e.target.value);
                      const clamped =
                        Number.isFinite(parsed) && parsed > 0
                          ? Math.min(parsed, MAX_FILE_SIZE_KB)
                          : 1;
                      patchOptions({ maxFileSizeKb: clamped });
                    }}
                  />
                  <span className="text-xs text-zinc-500">KB</span>
                </label>
              </div>
            </div>
          </m.section>
        )}
      </AnimatePresence>

      {/* Pack button */}
      <m.button
        type="button"
        className={`flex w-full items-center justify-center gap-2 rounded-xl py-3.5 text-sm font-semibold transition-all duration-200 ${
          isRunning || !isValidTarget
            ? "cursor-not-allowed bg-primary/20 text-primary/50"
            : "bg-primary text-primary-foreground shadow-lg shadow-black/30 hover:bg-primary/90"
        }`}
        onClick={() => runPack()}
        disabled={isRunning || !isValidTarget}
        aria-busy={isRunning}
        whileTap={!isRunning && isValidTarget ? { scale: 0.98 } : undefined}
      >
        {isRunning ? (
          <>
            <m.span
              aria-hidden="true"
              animate={{ rotate: 360 }}
              transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
            >
              <LoaderIcon size={16} />
            </m.span>
            Packing…
          </>
        ) : (
          <>
            <PlayIcon size={16} />
            Pack
            <kbd className="ml-0.5 rounded border border-black/20 bg-black/10 px-1.5 py-0.5 font-sans text-[10px] font-medium leading-none opacity-70">
              {isMacPlatform() ? "⌘↵" : "Ctrl↵"}
            </kbd>
          </>
        )}
      </m.button>

      {/* Error banner */}
      <AnimatePresence>
        {errorMsg && (
          <m.div
            role="alert"
            className="flex items-start gap-3 rounded-xl border border-red-600/40 bg-red-950/40 px-4 py-3 text-sm text-red-300"
            initial={{ opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -8 }}
          >
            <AlertIcon size={16} className="mt-0.5 shrink-0 text-red-400" />
            <div className="flex-1 break-words">{errorMsg}</div>
            <button
              type="button"
              onClick={dismissError}
              aria-label="Dismiss error"
              className="-mr-1 -mt-1 shrink-0 rounded p-1 text-red-300/80 transition-colors hover:bg-red-900/40 hover:text-red-200"
            >
              <XIcon size={14} />
            </button>
          </m.div>
        )}
      </AnimatePresence>

      {/* Cancelled notice */}
      <AnimatePresence>
        {isCancelled && (
          <m.div
            role="status"
            className="flex items-center gap-3 rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3 text-sm text-zinc-400"
            initial={prefersReducedMotion ? false : { opacity: 0, y: -8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
          >
            <XIcon size={16} className="shrink-0 text-zinc-500" />
            Pack cancelled. Adjust your options and pack again whenever you're
            ready.
          </m.div>
        )}
      </AnimatePresence>

      {/* Last-run log — spec: after an error/cancel the ProgressLog stays
          reachable from Home until the next pack starts. No churn concern:
          events only update while running, and running renders Packing. */}
      {(status === "error" || status === "cancelled") && events.length > 0 && (
        <details className="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3">
          <summary className="cursor-pointer select-none text-xs font-semibold text-zinc-400">
            Last run log
          </summary>
          <div className="pt-3">
            <ProgressLog events={events} />
          </div>
        </details>
      )}

      {/* Recents */}
      {recentTargets.length > 0 && (
        <section className="space-y-2.5">
          <div className="text-center text-[10px] font-semibold uppercase tracking-[0.18em] text-zinc-600">
            Recent
          </div>
          <div className="flex flex-wrap justify-center gap-2">
            {recentTargets.map((r) => (
              <button
                key={`${r.kind}:${r.value}`}
                type="button"
                title={r.value}
                onClick={() => patchOptions({ target: { ...r } })}
                className="flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-left text-xs text-zinc-300 transition-colors hover:border-primary/40 hover:text-zinc-100"
              >
                {r.kind === "github" ? (
                  <GithubIcon size={12} className="text-primary" />
                ) : (
                  <FolderIcon size={12} className="text-primary" />
                )}
                {recentLabel(r.value, r.kind)}
              </button>
            ))}
          </div>
        </section>
      )}

      <AnimatePresence>{showOnboarding && <OnboardingCard />}</AnimatePresence>
    </m.div>
  );
}
```

- [ ] **Step 2: Check `ChevronDownIcon` exists**

Run: `grep -n "ChevronDownIcon" frontend/src/components/pack/icons.tsx`
If missing, add to `icons.tsx` following the existing re-export pattern in
that file (it re-exports lucide icons under local names), e.g.
`export { ChevronDown as ChevronDownIcon } from "lucide-react";` — match
the file's existing export style exactly.

- [ ] **Step 3: Verify + commit**

Run: `cd frontend && pnpm typecheck`
Expected: exit 0.

```bash
git add frontend/src/routes/Home.tsx frontend/src/components/pack/icons.tsx
git commit -m "feat(ui): Home moment"
```

---

### Task 9: `routes/Packing.tsx`

**Files:**
- Create: `frontend/src/routes/Packing.tsx`

- [ ] **Step 1: Create the component**

```tsx
import * as m from "framer-motion/m";
import { useMemo } from "react";
import { LoaderIcon, XIcon } from "../components/pack/icons";
import { PackProgressBar } from "../components/pack/PackProgressBar";
import { PhaseBreakdown } from "../components/pack/PhaseBreakdown";
import { ProgressLog } from "../components/pack/ProgressLog";
import { fadeUp } from "../lib/motion";
import { progressFromEvents } from "../lib/pack-progress";
import { useApp, useLastStats, usePackEvents } from "../lib/store";
import { useKeyboardShortcuts } from "../lib/use-keyboard-shortcuts";
import { usePackJob } from "../lib/use-pack-job";

/** Full-screen progress theater. The ONLY moment subscribed to the
 * high-churn events slice. Esc cancels (unless a sheet is open — the
 * global Esc handler closes that first). */
export default function Packing() {
  const events = usePackEvents();
  const lastStats = useLastStats();
  const target = useApp((s) => s.options.target.value);
  const { cancel, isRunning } = usePackJob();
  const progress = useMemo(() => progressFromEvents(events), [events]);

  useKeyboardShortcuts({
    escape: () => {
      if (useApp.getState().activeSheet) return;
      if (isRunning) cancel();
    },
  });

  return (
    <m.div
      className="mx-auto w-full max-w-2xl space-y-6 px-6 pb-16 pt-12"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="text-center">
        <div className="flex items-center justify-center gap-2 text-xs font-semibold uppercase tracking-wider text-primary">
          <m.span
            aria-hidden="true"
            animate={{ rotate: 360 }}
            transition={{ duration: 1.5, repeat: Infinity, ease: "linear" }}
          >
            <LoaderIcon size={13} />
          </m.span>
          Packing
        </div>
        <h1 className="mt-2 truncate font-mono text-sm text-zinc-400">
          {target}
        </h1>
      </div>

      <PackProgressBar value={progress} />
      <ProgressLog events={events} />
      {lastStats && <PhaseBreakdown stats={lastStats} />}

      <div className="flex justify-center">
        <m.button
          type="button"
          onClick={() => cancel()}
          title="Cancel pack (Esc)"
          className="flex items-center gap-2 rounded-xl border border-red-600/50 bg-red-950/40 px-5 py-3 text-sm font-semibold text-red-300 transition-colors hover:bg-red-900/40 hover:text-red-200"
          whileTap={{ scale: 0.97 }}
        >
          <XIcon size={15} />
          Cancel
        </m.button>
      </div>
    </m.div>
  );
}
```

- [ ] **Step 2: Verify + commit**

Run: `cd frontend && pnpm typecheck`
Expected: exit 0.

```bash
git add frontend/src/routes/Packing.tsx
git commit -m "feat(ui): Packing moment"
```

---

### Task 10: `routes/Results.tsx` + `FitsCard`

**Files:**
- Create: `frontend/src/components/results/FitsCard.tsx`
- Create: `frontend/src/routes/Results.tsx`

- [ ] **Step 1: Create `FitsCard.tsx`**

```tsx
import type { TokensPerModel } from "../../bindings";
import { fmtNum } from "../../lib/format";
import { type Fit, fitsIn, MODEL_WINDOWS } from "../../lib/model-windows";

const FIT_STYLES: Record<Fit, { label: string; cls: string }> = {
  fits: { label: "✓ fits", cls: "text-success" },
  tight: { label: "◐ tight", cls: "text-warning" },
  over: { label: "✗ over", cls: "text-red-400" },
};

/** "Fits in" table: per-model token count vs advertised context window.
 * Replaces the old AiContextTable. Hidden entirely when token counting
 * was disabled for the pack. */
export function FitsCard({
  tokensPerModel,
}: {
  tokensPerModel: TokensPerModel | null;
}) {
  if (!tokensPerModel) return null;
  return (
    <section className="rounded-xl border border-white/10 bg-white/[0.03] p-5">
      <div className="mb-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-zinc-500">
        Fits in
      </div>
      <table className="w-full text-left text-sm">
        <tbody>
          {MODEL_WINDOWS.map(({ key, label, window }) => {
            const tokens = tokensPerModel[key];
            const fit = fitsIn(tokens, window);
            const style = FIT_STYLES[fit];
            return (
              <tr key={key} className="border-b border-white/5 last:border-0">
                <td className="py-2 text-zinc-200">
                  {label}{" "}
                  <span className={`ml-1.5 text-xs ${style.cls}`}>
                    {style.label}
                  </span>
                </td>
                <td className="nums py-2 text-right font-mono text-xs text-zinc-400">
                  {fmtNum(tokens)} / {fmtNum(window)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
```

- [ ] **Step 2: Create `Results.tsx`**

This merges the old ResultsTab deep dive with a stats hero. Copy the
warnings, redactions, and output-preview sections **verbatim** from the old
`routes/Pack.tsx` `ResultsTab` (lines 1109–1222) into the marked slots —
they are not repeated here because they move unchanged except: replace
`<SectionTitle>` import path with `../components/home/SectionTitle`.

```tsx
import { AnimatePresence } from "framer-motion";
import * as m from "framer-motion/m";
import { useState } from "react";
import { SectionTitle } from "../components/home/SectionTitle";
import { CompressionPanel } from "../components/pack/CompressionPanel";
import { CopyButton } from "../components/pack/CopyButton";
import {
  AlertIcon,
  BridgeIcon,
  CheckIcon,
  FileTextIcon,
  PackageIcon,
} from "../components/pack/icons";
import { PhaseBreakdown } from "../components/pack/PhaseBreakdown";
import { SaveButton } from "../components/pack/SaveButton";
import { FitsCard } from "../components/results/FitsCard";
import { fmtBytes, fmtNum } from "../lib/format";
import { COPY_BUTTON_LABELS, SAVE_FILENAMES } from "../lib/pack-meta";
import { fadeUp, prefersReducedMotion, springButton, springQuick } from "../lib/motion";
import { clampList } from "../lib/paginate";
import { useApp, usePackOptions, usePackProgress } from "../lib/store";

export default function Results() {
  const { options } = usePackOptions();
  const { result } = usePackProgress();
  const reset = useApp((s) => s.reset);
  const setMoment = useApp((s) => s.setMoment);
  const [showAllWarnings, setShowAllWarnings] = useState(false);
  const [showAllRedactions, setShowAllRedactions] = useState(false);

  if (!result) {
    return (
      <div className="mx-auto flex min-h-[60vh] w-full max-w-2xl flex-col items-center justify-center px-6 text-center">
        <FileTextIcon size={28} className="text-zinc-500" />
        <h3 className="mt-4 text-base font-semibold text-zinc-200">
          No pack results yet
        </h3>
        <m.button
          type="button"
          className="mt-5 flex items-center gap-2 rounded-lg bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground"
          onClick={() => setMoment("home")}
          whileTap={springButton}
        >
          <PackageIcon size={14} />
          Back to Home
        </m.button>
      </div>
    );
  }

  const previewLimit = 8000;
  const preview = result.output.slice(0, previewLimit);
  const truncated = result.output.length > previewLimit;
  const warnings = clampList(result.warnings, showAllWarnings);
  const redactions = clampList(result.redactions, showAllRedactions);
  const stats = result.stats;

  return (
    <m.div
      className="mx-auto w-full max-w-3xl space-y-6 px-6 pb-16 pt-8"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      {/* Crumb */}
      <div className="flex items-center gap-2 font-mono text-xs text-zinc-500">
        <span className="truncate">{options.target.value}</span>
        <span aria-hidden="true">·</span>
        <span>{options.format}</span>
        <span aria-hidden="true">·</span>
        <span>{fmtNum(stats.durationMs)} ms</span>
        <button
          type="button"
          onClick={() => reset()}
          className="ml-auto shrink-0 text-zinc-400 underline-offset-2 transition-colors hover:text-zinc-100 hover:underline"
        >
          ← new pack
        </button>
      </div>

      {/* Hero */}
      <section className="space-y-1.5">
        <div className="flex items-center gap-2">
          <m.span
            aria-hidden="true"
            className="flex h-5 w-5 items-center justify-center rounded-full bg-success/15 text-success ring-1 ring-success/25"
            initial={prefersReducedMotion ? false : { scale: 0, rotate: -90 }}
            animate={{ scale: 1, rotate: 0 }}
            transition={
              prefersReducedMotion
                ? { duration: 0 }
                : { ...springQuick, delay: 0.15 }
            }
          >
            <CheckIcon size={12} strokeWidth={2} />
          </m.span>
          <span className="text-xs font-semibold uppercase tracking-wider text-success">
            Pack complete
          </span>
        </div>
        <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
          <span className="nums text-4xl font-bold tracking-tight text-primary">
            {stats.tokensTotal != null
              ? fmtNum(stats.tokensTotal)
              : fmtNum(stats.filesIncluded)}
          </span>
          <span className="text-sm text-zinc-400">
            {stats.tokensTotal != null ? "tokens · " : ""}
            {fmtNum(stats.filesIncluded)} files · {fmtBytes(stats.bytesTotal)}
            {stats.secretsFound > 0
              ? ` · ${fmtNum(stats.secretsFound)} secrets redacted`
              : ""}
          </span>
        </div>
      </section>

      {/* Actions */}
      <section className="flex flex-wrap gap-2.5">
        <CopyButton
          label={COPY_BUTTON_LABELS[options.format]}
          text={result.output}
        />
        <SaveButton
          label="Save to file…"
          suggestedFilename={SAVE_FILENAMES[options.format]}
          text={result.output}
        />
        <CopyButton label="Copy Executor Prompt" text={result.executorPrompt} />
      </section>

      <FitsCard tokensPerModel={stats.tokensPerModel} />
      <PhaseBreakdown stats={stats} />
      <CompressionPanel />

      {/* Bridge banner */}
      <button
        type="button"
        onClick={() => setMoment("bridge")}
        className="flex w-full items-center justify-between gap-4 rounded-xl border border-primary/30 bg-gradient-to-r from-primary/10 to-transparent px-5 py-4 text-left transition-colors hover:border-primary/50"
      >
        <span>
          <span className="flex items-center gap-2 text-sm font-semibold text-zinc-100">
            <BridgeIcon size={15} className="text-primary" />
            Got a plan back from your planner?
          </span>
          <span className="mt-1 block text-xs text-zinc-500">
            Validate it against plan-exec-v1 and build the executor prompt.
          </span>
        </span>
        <span className="shrink-0 text-sm font-bold text-primary">
          Open Bridge →
        </span>
      </button>

      {/* Warnings — paste the old ResultsTab warnings section here verbatim */}
      {/* Redactions — paste the old ResultsTab redactions section here verbatim */}
      {/* Output preview — paste the old ResultsTab preview section here verbatim */}
      <AnimatePresence>{null}</AnimatePresence>
    </m.div>
  );
}
```

After pasting the three verbatim sections, delete the placeholder
`<AnimatePresence>{null}</AnimatePresence>` line and the three comment
markers. The pasted sections reference `AlertIcon`, `warnings`,
`redactions`, `setShowAllWarnings`, `setShowAllRedactions`, `fmtNum`,
`fmtBytes`, `SectionTitle`, `previewLimit`, `preview`, `truncated` — all
already imported/defined above.

- [ ] **Step 3: Verify + commit**

Run: `cd frontend && pnpm typecheck`
Expected: exit 0.

```bash
git add frontend/src/components/results/ frontend/src/routes/Results.tsx
git commit -m "feat(ui): Results moment with stats hero + FitsCard"
```

---

### Task 11: `routes/Bridge.tsx`

**Files:**
- Create: `frontend/src/routes/Bridge.tsx`

- [ ] **Step 1: Create the component**

```tsx
import * as m from "framer-motion/m";
import { BridgeTab } from "../components/bridge/BridgeTab";
import { fadeUp } from "../lib/motion";
import { useApp, usePackProgress } from "../lib/store";

/** Final stage of the journey: paste plan → validate → executor prompt.
 * Wraps the existing BridgeTab (logic + tests unchanged). */
export default function Bridge() {
  const setMoment = useApp((s) => s.setMoment);
  const { result } = usePackProgress();

  return (
    <m.div
      className="mx-auto w-full max-w-3xl space-y-5 px-6 pb-16 pt-8"
      variants={fadeUp}
      initial="hidden"
      animate="visible"
    >
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-primary">
            Bridge
          </p>
          <h1 className="mt-1 text-2xl font-bold tracking-tight">
            Validate a plan, build the executor prompt
          </h1>
        </div>
        <button
          type="button"
          onClick={() => setMoment(result ? "results" : "home")}
          className="text-sm text-zinc-400 underline-offset-2 transition-colors hover:text-zinc-100 hover:underline"
        >
          ← back
        </button>
      </div>
      <BridgeTab />
    </m.div>
  );
}
```

- [ ] **Step 2: Verify + commit**

Run: `cd frontend && pnpm typecheck`
Expected: exit 0.

```bash
git add frontend/src/routes/Bridge.tsx
git commit -m "feat(ui): Bridge moment"
```

---

### Task 12: Shell swap — new App.tsx, delete Pack.tsx + AiContextTable, ⌘K

**Files:**
- Modify: `frontend/src/App.tsx`
- Delete: `frontend/src/routes/Pack.tsx`
- Delete: `frontend/src/components/pack/AiContextTable.tsx`

- [ ] **Step 1: Rewrite the `App()` component and add `MomentView`**

In `frontend/src/App.tsx`: keep `useSystemTheme`, `useSingleInstance`,
`TOAST_KIND_STYLES`, `Toaster`, `ErrorFallback` unchanged. Replace the
`import Pack from "./routes/Pack";` line and the `App` export with:

```tsx
import { AnimatePresence } from "framer-motion";
import { Settings } from "./components/pack/Settings";
import { DropOverlay } from "./components/pack/DropOverlay";
import { GithubConnector } from "./components/pack/GithubConnector";
import { Sheet } from "./components/shell/Sheet";
import { TitleBar } from "./components/shell/TitleBar";
import { useDragDrop } from "./lib/use-drag-drop";
import { useKeyboardShortcuts } from "./lib/use-keyboard-shortcuts";
import Bridge from "./routes/Bridge";
import Home from "./routes/Home";
import Packing from "./routes/Packing";
import Results from "./routes/Results";
```

(merge with the existing imports — `AnimatePresence`, `useApp`, etc. are
already imported; add only what's missing.)

```tsx
const MOMENTS = {
  home: Home,
  packing: Packing,
  results: Results,
  bridge: Bridge,
} as const;

function MomentView() {
  const moment = useApp((s) => s.moment);
  const Active = MOMENTS[moment];
  return (
    <AnimatePresence mode="wait">
      <m.div
        key={moment}
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -12 }}
        transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
      >
        <Active />
      </m.div>
    </AnimatePresence>
  );
}

function Shell() {
  const activeSheet = useApp((s) => s.activeSheet);
  const setSheet = useApp((s) => s.setSheet);
  const setMoment = useApp((s) => s.setMoment);
  const patchOptions = useApp((s) => s.patchOptions);
  const status = useApp((s) => s.status);

  const { isDragging, dropState } = useDragDrop({
    onDrop: (folderPath: string) => {
      if (useApp.getState().status === "running") return;
      patchOptions({ target: { kind: "folder", value: folderPath } });
      setMoment("home");
    },
  });

  useKeyboardShortcuts({
    "mod+k": () => {
      setSheet(null);
      if (useApp.getState().status !== "running") setMoment("home");
      requestAnimationFrame(() =>
        document.getElementById("target-input")?.focus(),
      );
    },
    escape: () => {
      if (useApp.getState().activeSheet) setSheet(null);
    },
  });

  return (
    <div className="min-h-screen text-zinc-100">
      <DropOverlay visible={isDragging && status !== "running"} dropState={dropState} />
      <TitleBar />
      <MomentView />
      <Sheet
        open={activeSheet === "github"}
        title="GitHub"
        onClose={() => setSheet(null)}
      >
        <GithubConnector
          onSelectRepo={(htmlUrl) => {
            patchOptions({ target: { kind: "github", value: htmlUrl } });
            setSheet(null);
            if (useApp.getState().status !== "running") setMoment("home");
          }}
          onGoToSettings={() => setSheet("settings")}
        />
      </Sheet>
      <Sheet
        open={activeSheet === "settings"}
        title="Settings"
        onClose={() => setSheet(null)}
      >
        <Settings />
      </Sheet>
    </div>
  );
}

export default function App() {
  useSystemTheme();
  useSingleInstance();
  return (
    <ErrorBoundary FallbackComponent={ErrorFallback}>
      <LazyMotion features={domAnimation} strict>
        <MotionConfig
          reducedMotion="user"
          transition={{ duration: 0.18, ease: [0.2, 0, 0, 1] }}
        >
          <Shell />
          <Toaster />
        </MotionConfig>
      </LazyMotion>
    </ErrorBoundary>
  );
}
```

- [ ] **Step 2: Give the target input the ⌘K focus id**

In `frontend/src/components/home/TargetSection.tsx`, add `id="target-input"`
to BOTH `<input>` elements (folder path and GitHub URL — only one renders
at a time, so the id stays unique in the DOM).

- [ ] **Step 3: Delete the dead files**

```bash
git rm frontend/src/routes/Pack.tsx frontend/src/components/pack/AiContextTable.tsx
```

Then run `grep -rn "AiContextTable\|routes/Pack" frontend/src/` — expected:
zero hits. If `SkeletonRow`, `StatsBar`, or icons lose their last consumer,
leave them — Task 13 sweeps; only delete if `pnpm lint` flags them.

- [ ] **Step 4: Verify everything**

Run: `cd frontend && pnpm typecheck && pnpm test && pnpm lint`
Expected: typecheck 0; all tests pass; lint shows only pre-existing errors
(the ProgressLog/GithubConnector/use-drag-drop set — line numbers may
shift).

- [ ] **Step 5: Smoke-run the app**

Run: `pnpm tauri dev` from the repo root (or `cd frontend && pnpm dev` for
the web shell) and verify: Home renders with chips + advanced disclosure;
packing a folder transitions Home → Packing → Results; Bridge opens from
the Results banner; ⌘K focuses the target field; GitHub/Settings sheets
open and close with Esc.

- [ ] **Step 6: Commit**

```bash
git add -A frontend/src
git commit -m "feat(ui): four-moment shell replaces tab navigation"
```

---

### Task 13: Brand sweep — ember tokens in surviving components

**Files (every file that still contains `emerald`):**
Run `grep -rln "emerald" frontend/src/` for the authoritative list;
expected: BridgeTab.tsx, GithubConnector.tsx, Settings.tsx,
CompressionPanel.tsx, TransformRow.tsx, StatsBar.tsx, ProgressLog.tsx,
PackProgressBar.tsx, PhaseBreakdown.tsx, CopyButton.tsx, SaveButton.tsx,
Toggle.tsx, DropOverlay.tsx, OnboardingCard.tsx, TargetSection.tsx,
App.tsx (toasts), BridgeTab.test.tsx (only if it asserts classes — it
doesn't; leave tests alone).

- [ ] **Step 1: Apply the mapping in every listed file**

Replace by **role**, not blindly:

| Current class pattern | Replacement | Applies to |
|---|---|---|
| `bg-emerald-600`, `hover:bg-emerald-500` (action buttons) | `bg-primary`, `hover:bg-primary/90` + change paired `text-white` to `text-primary-foreground` | brand actions |
| `text-emerald-400` / `text-emerald-300` (accents, icons, labels) | `text-primary` | brand accents |
| `bg-emerald-500/10` … `/20` + `ring-emerald-500/20` … `/30` (badges, icon chips) | `bg-primary/10` etc. + `ring-primary/20` etc. (same alpha) | brand chips |
| `border-emerald-*`, `focus:border-emerald-500/50` | `border-primary/…`, `focus:border-primary/50` (same alpha) | inputs/borders |
| `shadow-emerald-900/30` etc. | `shadow-black/30` | button shadows |
| **Success semantics** — "Connected" badges (Settings.tsx:115, GithubConnector), save-success banners (Settings.tsx:212, 251), BridgeTab validation-passed states, success toast in App.tsx | `text-success`, `bg-success/15`, `ring-success/25`, `border-success/40` | KEEP GREEN |

Judgment rule: if the element communicates "an operation succeeded /
something is valid", it maps to `success`; everything else (brand
identity, primary actions, active states, focus) maps to `primary`.

- [ ] **Step 2: Verify zero stray emerald + suite**

Run: `grep -rn "emerald" frontend/src/ --include="*.tsx" --include="*.ts" | grep -v test`
Expected: zero hits.
Run: `cd frontend && pnpm typecheck && pnpm test && pnpm lint`
Expected: green / only pre-existing lint errors.

- [ ] **Step 3: Visual smoke**

`pnpm tauri dev`: confirm ember accents everywhere, green only on
success/validation, red only on errors/secrets.

- [ ] **Step 4: Commit**

```bash
git add -A frontend/src
git commit -m "feat(ui): ember brand sweep; green reserved for success semantics"
```

---

### Task 14: Final verification gate

- [ ] **Step 1: Full frontend gate**

Run: `cd frontend && pnpm typecheck && pnpm test && pnpm lint`
Expected: typecheck 0; ALL tests pass (≥ 41: 24 existing + ~17 new);
lint = only the pre-existing error set.

- [ ] **Step 2: Rust untouched check**

Run: `git diff --stat refactor/simplification-pass@{u} -- crates/` (or
`git status crates/`)
Expected: zero changes under `crates/`.
Run: `cargo test -p projectpacker-core 2>&1 | tail -3`
Expected: all pass (unchanged code, sanity only).

- [ ] **Step 3: End-to-end smoke**

`pnpm tauri dev`: pack this repo itself; walk Home → Packing → Results →
Bridge; paste a malformed plan in Bridge and confirm red validation items;
copy executor prompt; open both sheets; toggle a preset chip and confirm
the Advanced panel reflects it.

- [ ] **Step 4: Push**

```bash
git push origin refactor/simplification-pass
```
```
