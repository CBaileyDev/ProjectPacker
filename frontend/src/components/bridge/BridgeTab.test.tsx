import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BridgeTab } from "./BridgeTab";

// The clipboard plugin talks to Tauri IPC, unavailable in happy-dom.
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn(() => Promise.resolve()),
}));

// Stub the two protocol commands; each test programs the resolved values.
const validatePlan = vi.fn();
const buildCombinedPrompt = vi.fn();
vi.mock("../../bindings", () => ({
  commands: {
    validatePlan: (...args: unknown[]) => validatePlan(...args),
    buildCombinedPrompt: (...args: unknown[]) => buildCombinedPrompt(...args),
  },
}));

// Replace the app store with a minimal real Zustand store holding only the
// Bridge slice — the real module pulls in the Tauri persist adapter, which
// isn't available in happy-dom. A live store (not a plain object) is needed
// so typing into the textarea re-renders the component.
vi.mock("../../lib/store", async () => {
  const { create } = await vi.importActual<typeof import("zustand")>("zustand");
  const useApp = create<{
    bridgePlanMd: string;
    setBridgePlanMd: (md: string) => void;
  }>((set) => ({
    bridgePlanMd: "",
    setBridgePlanMd: (md) => set({ bridgePlanMd: md }),
  }));
  return { useApp };
});

import { useApp } from "../../lib/store";

beforeEach(() => {
  useApp.setState({ bridgePlanMd: "" });
  validatePlan.mockReset();
  buildCombinedPrompt.mockReset();
});

describe("BridgeTab", () => {
  it("renders validator errors and a re-prompt button for an invalid plan", async () => {
    validatePlan.mockResolvedValue({
      status: "ok",
      data: {
        ok: false,
        errors: [
          { code: "missing_section", message: "Missing section: ### Risks" },
        ],
      },
    });

    render(<BridgeTab />);
    fireEvent.change(screen.getByRole("textbox", { name: /plan markdown/i }), {
      target: { value: "### Summary\nnot a real plan" },
    });
    fireEvent.click(screen.getByRole("button", { name: /validate plan/i }));

    expect(await screen.findByText("missing_section")).toBeTruthy();
    expect(screen.getByText(/Missing section: ### Risks/)).toBeTruthy();
    expect(
      screen.getByRole("button", { name: /copy re-prompt/i }),
    ).toBeTruthy();
    // A rejected plan must never reach the combined-prompt builder.
    expect(buildCombinedPrompt).not.toHaveBeenCalled();
  });

  it("offers the combined prompt for a valid plan", async () => {
    validatePlan.mockResolvedValue({
      status: "ok",
      data: { ok: true, errors: [] },
    });
    buildCombinedPrompt.mockResolvedValue({ status: "ok", data: "COMBINED" });

    render(<BridgeTab />);
    fireEvent.change(screen.getByRole("textbox", { name: /plan markdown/i }), {
      target: { value: "### Summary\na valid plan" },
    });
    fireEvent.click(screen.getByRole("button", { name: /validate plan/i }));

    expect(
      await screen.findByRole("button", { name: /copy combined prompt/i }),
    ).toBeTruthy();
    expect(screen.getByText(/plan is valid/i)).toBeTruthy();
    expect(buildCombinedPrompt).toHaveBeenCalledWith(
      "### Summary\na valid plan",
      "plan-exec-v1",
    );
  });

  it("clears a stale result when the plan is edited", async () => {
    validatePlan.mockResolvedValue({
      status: "ok",
      data: { ok: true, errors: [] },
    });
    buildCombinedPrompt.mockResolvedValue({ status: "ok", data: "COMBINED" });

    render(<BridgeTab />);
    const textarea = screen.getByRole("textbox", { name: /plan markdown/i });
    fireEvent.change(textarea, { target: { value: "### Summary\nv1" } });
    fireEvent.click(screen.getByRole("button", { name: /validate plan/i }));
    await screen.findByRole("button", { name: /copy combined prompt/i });

    fireEvent.change(textarea, { target: { value: "### Summary\nv2" } });
    expect(
      screen.queryByRole("button", { name: /copy combined prompt/i }),
    ).toBeNull();
  });
});
