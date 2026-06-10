import type { TokensPerModel } from "../bindings";

/** Advertised context windows (tokens), deliberately conservative round
 * numbers — this powers a fits/tight/over hint, not billing. Update when
 * vendors move the goalposts. */
export interface ModelWindow {
  key: keyof TokensPerModel;
  label: string;
  window: number;
}

export const MODEL_WINDOWS = [
  { key: "claude", label: "Claude", window: 200_000 },
  { key: "geminiApprox", label: "Gemini (approx)", window: 1_000_000 },
  { key: "gpt4o", label: "GPT-4o", window: 128_000 },
  { key: "llama3", label: "Llama 3", window: 128_000 },
  { key: "qwen2_5", label: "Qwen 2.5", window: 128_000 },
  { key: "deepSeek", label: "DeepSeek", window: 128_000 },
  { key: "mistral", label: "Mistral (Large)", window: 128_000 },
] as const satisfies readonly ModelWindow[];

type Covered = (typeof MODEL_WINDOWS)[number]["key"];
// Compile error here = a TokensPerModel key is missing from the table.
type _Exhaustive = [Exclude<keyof TokensPerModel, Covered>] extends [never]
  ? true
  : never;

export type Fit = "fits" | "tight" | "over";

/** Utilisation above this fraction of the window is flagged "tight". */
export const TIGHT_THRESHOLD = 0.75;

/** Strictly-over-the-window is "over"; above 75% utilisation is "tight". */
export function fitsIn(tokens: number, window: number): Fit {
  if (tokens > window) return "over";
  if (tokens > window * TIGHT_THRESHOLD) return "tight";
  return "fits";
}
