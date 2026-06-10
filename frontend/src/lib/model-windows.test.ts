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

  it("every row has sane app and API windows", () => {
    for (const m of MODEL_WINDOWS) {
      expect(m.appWindow, m.label).toBeGreaterThan(0);
      expect(m.apiWindow, m.label).toBeGreaterThanOrEqual(m.appWindow);
    }
  });

  it("reflects the verified June 2026 flagship windows", () => {
    const byKey = Object.fromEntries(MODEL_WINDOWS.map((m) => [m.key, m]));
    // Claude Code / Anthropic API is 1M; the claude.ai paid app is 500K
    // on Opus 4.8 (Fable 5: 200K) — the user-facing pain this table
    // exists to prevent is "Claude can't handle my pack" when Claude
    // Code handles it fine.
    expect(byKey.claude.apiWindow).toBe(1_000_000);
    expect(byKey.gpt4o.apiWindow).toBe(1_050_000);
  });
});
