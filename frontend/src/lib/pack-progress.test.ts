import { describe, expect, it } from "vitest";
import type { PackStats, ProgressEvent } from "../bindings";
import { progressForEvent, progressFromEvents } from "./pack-progress";

function stats(): PackStats {
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

describe("progressForEvent", () => {
  it("maps started to 0", () => {
    expect(
      progressForEvent({ kind: "started", job_id: "j", target_label: "x" }),
    ).toBe(0);
  });

  it("maps cloning into 0–10", () => {
    expect(progressForEvent({ kind: "cloning", progress_pct: 0 })).toBe(0);
    expect(progressForEvent({ kind: "cloning", progress_pct: 50 })).toBe(5);
    expect(progressForEvent({ kind: "cloning", progress_pct: 100 })).toBe(10);
  });

  it("keeps walking inside 0–40 and strictly increasing with file count", () => {
    const at = (n: number) =>
      progressForEvent({ kind: "walking", files_scanned: n }) as number;
    expect(at(0)).toBe(0);
    expect(at(500)).toBe(20); // halfway point of the asymptote
    expect(at(100)).toBeGreaterThan(at(10));
    expect(at(1_000_000)).toBeLessThan(40);
  });

  it("maps tokenizing into 40–60", () => {
    expect(progressForEvent({ kind: "tokenizing", progress_pct: 0 })).toBe(40);
    expect(progressForEvent({ kind: "tokenizing", progress_pct: 50 })).toBe(50);
    expect(progressForEvent({ kind: "tokenizing", progress_pct: 100 })).toBe(
      60,
    );
  });

  it("maps secret scanning into 60–80", () => {
    expect(progressForEvent({ kind: "secretScanning", progress_pct: 0 })).toBe(
      60,
    );
    expect(
      progressForEvent({ kind: "secretScanning", progress_pct: 100 }),
    ).toBe(80);
  });

  it("maps compressing into 80–95", () => {
    expect(progressForEvent({ kind: "compressing", progress_pct: 0 })).toBe(80);
    expect(progressForEvent({ kind: "compressing", progress_pct: 100 })).toBe(
      95,
    );
  });

  it("anchors emit at 95 and done at 100", () => {
    expect(progressForEvent({ kind: "buildingOutput" })).toBe(95);
    expect(progressForEvent({ kind: "done", stats: stats() })).toBe(100);
  });

  it("clamps out-of-range phase percentages", () => {
    expect(progressForEvent({ kind: "tokenizing", progress_pct: 150 })).toBe(
      60,
    );
    expect(progressForEvent({ kind: "tokenizing", progress_pct: -5 })).toBe(40);
  });

  it("returns null for events without a phase position", () => {
    expect(progressForEvent({ kind: "fileFoundBatch", paths: [] })).toBeNull();
    expect(
      progressForEvent({
        kind: "secretHit",
        path: "a",
        secret_kind: "k",
        line: 1,
      }),
    ).toBeNull();
    expect(
      progressForEvent({ kind: "error", message: "boom", fatal: false }),
    ).toBeNull();
  });
});

describe("progressFromEvents", () => {
  it("returns 0 for an empty log", () => {
    expect(progressFromEvents([])).toBe(0);
  });

  it("tracks the furthest phase seen, ignoring stragglers", () => {
    const events: ProgressEvent[] = [
      { kind: "started", job_id: "j", target_label: "x" },
      { kind: "walking", files_scanned: 500 },
      { kind: "tokenizing", progress_pct: 50 },
      // A late walking tick must not pull the bar backwards.
      { kind: "walking", files_scanned: 600 },
    ];
    expect(progressFromEvents(events)).toBe(50);
  });

  it("fills to 100 on done", () => {
    const events: ProgressEvent[] = [
      { kind: "compressing", progress_pct: 60 },
      { kind: "done", stats: stats() },
    ];
    expect(progressFromEvents(events)).toBe(100);
  });
});
