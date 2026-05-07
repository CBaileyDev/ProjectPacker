import type { TokenModel } from "../bindings";

export type ModelRow = {
  name: string;
  context: number;
  tokenModel: TokenModel;
  /** True when our tokenizer is an approximation, not the model's authentic
   * tokenizer. Renders an "approx" badge and is called out in the footer. */
  approx?: boolean;
};

export const AI_MODELS: ModelRow[] = [
  { name: "GPT-4o / GPT-4o mini", context: 128_000, tokenModel: "gpt4o" },
  {
    name: "Claude 3.x / Claude 4.x",
    context: 200_000,
    tokenModel: "claude",
    approx: true, // Anthropic's tokenizer is unpublished; we use cl100k as a proxy.
  },
  { name: "o1 / o3", context: 200_000, tokenModel: "gpt4o" },
  { name: "DeepSeek V3", context: 128_000, tokenModel: "deepSeek" },
  { name: "Llama 3.x (70B+)", context: 128_000, tokenModel: "llama3" },
  { name: "Qwen 2.5 (7B+)", context: 128_000, tokenModel: "qwen2_5" },
  { name: "Mistral 7B / Mixtral", context: 32_768, tokenModel: "mistral" },
  {
    name: "Grok 2 / 3",
    context: 131_072,
    tokenModel: "gpt4o",
    approx: true, // xAI's tokenizer is unpublished; cl100k is a proxy.
  },
  {
    name: "Gemini 1.5 Pro",
    context: 1_048_576,
    tokenModel: "geminiApprox",
    approx: true,
  },
  {
    name: "Gemini 2.0 Flash",
    context: 1_048_576,
    tokenModel: "geminiApprox",
    approx: true,
  },
  {
    name: "Gemini 2.5 Pro",
    context: 1_048_576,
    tokenModel: "geminiApprox",
    approx: true,
  },
];
