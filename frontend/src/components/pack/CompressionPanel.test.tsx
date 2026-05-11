import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { PackOptions } from "../../bindings";
import { CompressionPanel } from "./CompressionPanel";

// The component reads from `usePackOptions` + `useLastStats` only. We mock
// both with a fixed conservative-default state (4 lossless on, 6 off) so
// the test is independent of the persist layer (which talks to Tauri's
// LazyStore and isn't available in happy-dom). `patchOptions` is a spy
// that mutates the local state object so the test can assert post-toggle
// behaviour if extended later.
vi.mock("../../lib/store", () => {
  const options: PackOptions = {
    target: { kind: "folder", value: "" },
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
    protocolVersion: "grok-to-cc-v1",
    format: "xml",
    xmlSchema: "cxml",
  };
  const patchOptions = vi.fn((p: Partial<PackOptions>) =>
    Object.assign(options, p),
  );
  const setOptions = vi.fn();
  return {
    usePackOptions: () => ({ options, patchOptions, setOptions }),
    useLastStats: () => null,
  };
});

describe("CompressionPanel", () => {
  it("expands when the header is clicked", () => {
    render(<CompressionPanel />);
    // Closed by default — the LOSSLESS group caption shouldn't be in the
    // DOM yet.
    expect(screen.queryByText(/LOSSLESS/)).toBeNull();

    // The header is the only button at the top level that contains the
    // word "Compression". Click it.
    fireEvent.click(screen.getByRole("button", { name: /Compression/ }));

    // After expanding, all three group headers should be present.
    expect(screen.getByText("LOSSLESS")).toBeTruthy();
    expect(screen.getByText("SEMANTIC")).toBeTruthy();
    expect(screen.getByText("CODE SHAPING")).toBeTruthy();
  });

  it("shows '4 of 10 enabled' for the conservative default", () => {
    render(<CompressionPanel />);
    expect(screen.getByText(/4 of 10 enabled/)).toBeTruthy();
  });
});
