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
