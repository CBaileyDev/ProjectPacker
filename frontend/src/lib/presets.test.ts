import { describe, expect, it } from "vitest";
import type { PackOptions } from "../bindings";
import { defaultOptions } from "./default-options";
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

  it("the hardcoded base fixture matches balanced", () => {
    expect(matchPreset(base)).toBe("balanced");
  });

  it("factory defaults match the balanced preset", () => {
    expect(matchPreset(defaultOptions)).toBe("balanced");
  });

  it("goal/target/format changes never disqualify a preset", () => {
    expect(matchPreset({ ...base, goal: "anything", format: "markdown" })).toBe(
      "balanced",
    );
  });
});
