import { describe, expect, it } from "vitest";
import { clampList, DEFAULT_LIST_CAP } from "./paginate";

const items = (n: number) => Array.from({ length: n }, (_, i) => i);

describe("clampList", () => {
  it("passes short lists through untouched", () => {
    const { visible, hiddenCount, isCapped } = clampList(items(3), false, 50);
    expect(visible).toHaveLength(3);
    expect(hiddenCount).toBe(0);
    expect(isCapped).toBe(false);
  });

  it("treats a list exactly at the cap as uncapped", () => {
    const { visible, hiddenCount, isCapped } = clampList(items(50), false, 50);
    expect(visible).toHaveLength(50);
    expect(hiddenCount).toBe(0);
    expect(isCapped).toBe(false);
  });

  it("caps long collapsed lists to the first `cap` entries", () => {
    const { visible, hiddenCount, isCapped } = clampList(items(120), false, 50);
    expect(visible).toHaveLength(50);
    expect(visible[0]).toBe(0);
    expect(visible[49]).toBe(49);
    expect(hiddenCount).toBe(70);
    expect(isCapped).toBe(true);
  });

  it("reveals everything when expanded but stays marked as capped", () => {
    const { visible, hiddenCount, isCapped } = clampList(items(120), true, 50);
    expect(visible).toHaveLength(120);
    expect(hiddenCount).toBe(0);
    expect(isCapped).toBe(true);
  });

  it("defaults the cap to 50", () => {
    expect(DEFAULT_LIST_CAP).toBe(50);
    expect(clampList(items(51), false).visible).toHaveLength(50);
  });
});
