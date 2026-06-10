import { beforeEach, describe, expect, it, vi } from "vitest";

// In-memory stand-in for the Tauri plugin-store IPC boundary — the only
// unavoidable mock. The adapter and sanitize logic under test are real.
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

import { tauriStoreAdapter } from "./persist-adapter";
import { PROTOCOL_VERSION } from "./protocol";

const blobWith = (options: Record<string, unknown>) =>
  JSON.stringify({ state: { options }, version: 0 });

describe("persist-adapter rehydration", () => {
  beforeEach(() => {
    backing.clear();
  });

  it("returns null when nothing is persisted", async () => {
    expect(await tauriStoreAdapter.getItem("app-state")).toBeNull();
  });

  it("migrates a retired protocolVersion forward on cold read", async () => {
    // A settings file written by a pre-rebrand build still carries the
    // retired version string. Rehydration must hand zustand the migrated
    // value — otherwise the live store packs with it and core rejects
    // the pack with "unknown protocol version: grok-to-cc-v1".
    backing.set(
      "app-state",
      blobWith({
        protocolVersion: "grok-to-cc-v1",
        maxFileSizeKb: 1024,
        format: "xml",
        goal: "",
      }),
    );

    const raw = await tauriStoreAdapter.getItem("app-state");
    const options = JSON.parse(raw as string).state.options;
    expect(options.protocolVersion).toBe(PROTOCOL_VERSION);
  });

  it("coerces bad advancedOpen / recentTargets on cold read", async () => {
    backing.set(
      "app-state",
      JSON.stringify({
        state: {
          options: {
            protocolVersion: PROTOCOL_VERSION,
            maxFileSizeKb: 1024,
            format: "xml",
            goal: "",
          },
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
});
