import type { TokensPerModel } from "../bindings";

/** One row of the "Fits in" table: the current flagship of each model
 * family, with its consumer-app and API/coding context windows kept
 * separate — they often differ by 4-5x, and "fits in Claude Code but not
 * the claude.ai app" is exactly the nuance a packer user needs. */
export interface ModelWindow {
  key: keyof TokensPerModel;
  label: string;
  /** Paid-tier context window in the vendor's consumer chat app, tokens. */
  appWindow: number;
  /** Context window via the API / coding tools, tokens. */
  apiWindow: number;
  /** One-line nuance, rendered as the row's tooltip. */
  hint?: string;
}

/** Verified 2026-06-10 against vendor docs plus an independent second
 * source per row. Windows are advertised totals, rounded the way vendors
 * round them — this powers a fits/tight/over hint, not billing. Update
 * when vendors move the goalposts; the keys stay pinned to the seven
 * tokenizers the Rust core counts with, so counts remain
 * nearest-tokenizer estimates for each family's newest models. */
export const MODEL_WINDOWS = [
  {
    key: "claude",
    label: "Claude (Fable 5 / Opus 4.8)",
    appWindow: 500_000,
    apiWindow: 1_000_000,
    hint: "claude.ai paid app: 500K on Opus 4.8 / Sonnet 4.6, 200K on Fable 5 · Claude Code & API: 1M",
  },
  {
    key: "gpt4o",
    label: "ChatGPT (GPT-5.5)",
    appWindow: 256_000,
    apiWindow: 1_050_000,
    hint: "ChatGPT paid app: 256K with Thinking selected (input capped at 128K on Plus, 272K on Pro) · API: 1.05M",
  },
  {
    key: "geminiApprox",
    label: "Gemini (3.1 Pro)",
    appWindow: 1_000_000,
    apiWindow: 1_000_000,
    hint: "Gemini app: 1M on AI Pro/Ultra plans (32K free) · API: 1M",
  },
  {
    key: "llama3",
    label: "Llama 4 (Maverick)",
    appWindow: 1_000_000,
    apiWindow: 1_000_000,
    hint: "Open weights: 1M documented (the Scout variant headlines 10M)",
  },
  {
    key: "qwen2_5",
    label: "Qwen (3.5)",
    appWindow: 262_144,
    apiWindow: 262_144,
    hint: "Open weights: 262K native, ~1M with YaRN RoPE scaling",
  },
  {
    key: "deepSeek",
    label: "DeepSeek (V4)",
    appWindow: 1_000_000,
    apiWindow: 1_000_000,
    hint: "DeepSeek app & API: 1M (V4 Pro/Flash)",
  },
  {
    key: "mistral",
    label: "Mistral (Medium 3.5)",
    appWindow: 256_000,
    apiWindow: 256_000,
    hint: "Le Chat & API: 256K (Medium 3.5 flagship)",
  },
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
