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
import { defaultOptions } from "./default-options";
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
    options: { ...defaultOptions },
    moment: "home",
    activeSheet: null,
    recentTargets: [],
  });
}

describe("store initializer", () => {
  it("boots into home with no sheet and empty recents", () => {
    // NOTE: must run before any test mutates the singleton store —
    // vitest runs tests in declaration order within a file.
    const s = useApp.getState();
    expect(s.moment).toBe("home");
    expect(s.activeSheet).toBeNull();
    expect(s.recentTargets).toEqual([]);
  });
});

describe("moment state machine", () => {
  beforeEach(resetStore);

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

  it("setMoment is an unguarded navigation setter", () => {
    useApp.getState().setMoment("bridge");
    expect(useApp.getState().moment).toBe("bridge");
    useApp.getState().setMoment("results");
    expect(useApp.getState().moment).toBe("results");
  });

  it("a stale terminal event does not yank the user out of bridge", () => {
    useApp.getState().setJob("job-1");
    useApp.getState().pushEvent(doneEvent);
    useApp.getState().setMoment("bridge");
    // A duplicate/stale done event arrives after the user navigated away:
    useApp.getState().pushEvent(doneEvent);
    expect(useApp.getState().moment).toBe("bridge");
    useApp.getState().pushEventsBatched([doneEvent]);
    expect(useApp.getState().moment).toBe("bridge");
  });

  it("a non-fatal error is informational: status and moment unchanged", () => {
    useApp.getState().setJob("job-1");
    useApp
      .getState()
      .pushEvent({ kind: "error", message: "transient", fatal: false });
    expect(useApp.getState().status).toBe("running");
    expect(useApp.getState().moment).toBe("packing");
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
    useApp
      .getState()
      .patchOptions({ target: { kind: "folder", value: "/repo-3" } });
    useApp.getState().setJob("job-7");

    const recents = useApp.getState().recentTargets;
    expect(recents.length).toBe(5);
    expect(recents[0]).toEqual({ kind: "folder", value: "/repo-3" });
    expect(recents.filter((r) => r.value === "/repo-3").length).toBe(1);
  });

  it("skips empty targets and keeps kind-distinct entries separate", () => {
    useApp.getState().patchOptions({ target: { kind: "folder", value: "" } });
    useApp.getState().setJob("job-empty");
    expect(useApp.getState().recentTargets).toEqual([]);
    useApp.getState().patchOptions({ target: { kind: "folder", value: "/x" } });
    useApp.getState().setJob("job-a");
    useApp.getState().patchOptions({ target: { kind: "github", value: "/x" } });
    useApp.getState().setJob("job-b");
    expect(useApp.getState().recentTargets.length).toBe(2);
  });
});
