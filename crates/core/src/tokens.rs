//! Token counting for various LLM tokenizers.
//!
//! Two public APIs live here:
//! - [`count`] (new in v0.3.0, dispatched via [`TokenModel`]) — the
//!   forward path that frontends will move to. Uses `cl100k_base` for the
//!   OpenAI / Claude / Gemini-approx variants per the Phase 2 plan, and
//!   vendored HuggingFace `tokenizer.json` files for the four open-weight
//!   model families (Llama 3, Qwen 2.5, DeepSeek, Mistral).
//! - [`count_by_name`] (legacy, string-keyed) — preserved for backwards
//!   compatibility with existing callers in the orchestrator. Uses
//!   `o200k_base` for `"gpt-4o-mini"` / `"gpt-4o"` / `"gpt-4"` to keep
//!   observable token counts identical to v0.2.0.
//!
//! ## Vendored HuggingFace tokenizers
//!
//! The four `tokenizer.json` files under `crates/core/assets/tokenizers/` are
//! embedded at compile time via [`include_bytes!`]. Together they add roughly
//! 25 MiB of read-only data to the final binary — a deliberate trade for
//! hermeticity: ProjectPacker never makes a network call at runtime to count
//! tokens, regardless of model.
//!
//! Each tokenizer is parsed lazily on first use into a process-wide
//! `OnceLock<Tokenizer>`. The cold parse takes on the order of tens of
//! milliseconds; subsequent calls for the same model are zero-overhead vs.
//! the underlying [`tokenizers::Tokenizer::encode`] cost. A user that never
//! switches to an HF-backed model never pays the parse cost at all.

use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;
use tokenizers::Tokenizer;

// Two distinct encoder caches: legacy callers must keep the o200k counts they
// snapshot-tested in v0.2.0, while the new `TokenModel` API uses cl100k.
static O200K_BASE: OnceLock<CoreBPE> = OnceLock::new();
static CL100K_BASE: OnceLock<CoreBPE> = OnceLock::new();

// Vendored HuggingFace `tokenizer.json` blobs paired with their lazy parse
// slots. Sources are documented in the commit that landed Phase 2 T2;
// ungated public mirrors only. Pairing the bytes with the slot in a single
// struct prevents the copy-paste foot-gun where a model could route the
// wrong bytes into the wrong slot.
struct VendoredHfTokenizer {
    name: &'static str,
    bytes: &'static [u8],
    slot: OnceLock<Tokenizer>,
}

impl VendoredHfTokenizer {
    const fn new(name: &'static str, bytes: &'static [u8]) -> Self {
        Self {
            name,
            bytes,
            slot: OnceLock::new(),
        }
    }

    /// Lazily parse the vendored `tokenizer.json` and encode `text`.
    ///
    /// The first call parses the JSON (tens of milliseconds for ~1–9 MiB
    /// inputs); subsequent calls hit the cached `Tokenizer`. We do not
    /// pre-warm at startup — a user who never switches to an HF model
    /// never pays this cost.
    fn count(&'static self, text: &str) -> CoreResult<u32> {
        let tok = self.slot.get_or_init(|| {
            Tokenizer::from_bytes(self.bytes).unwrap_or_else(|e| {
                panic!("vendored {} tokenizer.json must parse: {e}", self.name)
            })
        });
        // `false` = no BOS/EOS — we count raw content tokens, not after-templating.
        let encoded = tok
            .encode(text, false)
            .map_err(|e| CoreError::TokenizerEncodeFailed(e.to_string()))?;
        Ok(encoded.get_ids().len() as u32)
    }
}

static LLAMA_3: VendoredHfTokenizer =
    VendoredHfTokenizer::new("llama-3", include_bytes!("../assets/tokenizers/llama-3.json"));
static QWEN_2_5: VendoredHfTokenizer =
    VendoredHfTokenizer::new("qwen-2.5", include_bytes!("../assets/tokenizers/qwen-2.5.json"));
static DEEPSEEK: VendoredHfTokenizer =
    VendoredHfTokenizer::new("deepseek", include_bytes!("../assets/tokenizers/deepseek.json"));
static MISTRAL: VendoredHfTokenizer =
    VendoredHfTokenizer::new("mistral", include_bytes!("../assets/tokenizers/mistral.json"));

/// Tokenizer family selector for the typed API.
///
/// Wire mapping:
/// - `Gpt4o`, `Claude` → `cl100k_base` (Anthropic's tokenizer behaves close
///   enough to cl100k that we share the encoder).
/// - `GeminiApprox` → `cl100k_base` count multiplied by 1.05 and rounded up;
///   Gemini ships no public tokenizer so this is a deliberate over-estimate.
/// - `Llama3`, `Qwen2_5`, `DeepSeek`, `Mistral` → vendored HuggingFace
///   `tokenizer.json` blobs, parsed lazily on first use (Phase 2 T2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TokenModel {
    // Explicit per-variant renames are used for `Gpt4o` and `Qwen2_5` because
    // specta-typescript v0.0.9 and serde disagree on the camelCase output for
    // those two variants (`"gpt4O"` vs `"gpt4o"`, `"qwen25"` vs `"qwen2_5"`),
    // which would break IPC deserialization. The renames below pin a single
    // wire string both producers agree on and double as ergonomic identifiers
    // for the frontend union type.
    #[serde(rename = "gpt4o")]
    Gpt4o,
    Claude,
    Llama3,
    #[serde(rename = "qwen2_5")]
    Qwen2_5,
    DeepSeek,
    Mistral,
    GeminiApprox,
}

/// All seven per-model token counts for a single input.
///
/// Returned by [`count_all`]; mirrors the rows in the AI compatibility table.
/// The wire-format field names match [`TokenModel`]'s variant strings so
/// frontends can subscript with `tokensPerModel[tokenModel]` at type-check
/// time. See `frontend/src/routes/Pack.tsx` for the consumer side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TokensPerModel {
    #[serde(rename = "gpt4o")]
    pub gpt4o: u32,
    pub claude: u32,
    pub llama3: u32,
    #[serde(rename = "qwen2_5")]
    pub qwen2_5: u32,
    pub deep_seek: u32,
    pub mistral: u32,
    pub gemini_approx: u32,
}

/// Tokenize `text` once per model and return the seven counts.
///
/// Internally `Gpt4o`, `Claude`, and `GeminiApprox` all share the cl100k
/// `OnceLock` encoder, so calling [`count`] three times for those variants
/// is cheap (one real encode plus two repeated encodes). The four HF
/// tokenizers each parse + encode once.
///
/// All counts are computed by routing through [`count`] rather than
/// open-coding the math here, so any future change to the per-model
/// encoder selection automatically picks up here too.
///
/// Errors from individual tokenizers propagate; in practice the only
/// failure mode is an internal HF tokenizer bug, which orchestrator
/// callers swallow via `.ok()` to keep packs non-fatal.
pub fn count_all(text: &str) -> CoreResult<TokensPerModel> {
    let cl100k = count(text, TokenModel::Gpt4o)?;
    Ok(TokensPerModel {
        gpt4o: cl100k,
        claude: cl100k, // shares cl100k via the OnceLock; this is the same number.
        gemini_approx: count(text, TokenModel::GeminiApprox)?,
        llama3: count(text, TokenModel::Llama3)?,
        qwen2_5: count(text, TokenModel::Qwen2_5)?,
        deep_seek: count(text, TokenModel::DeepSeek)?,
        mistral: count(text, TokenModel::Mistral)?,
    })
}

/// Count tokens for `text` using the encoder family selected by `model`.
pub fn count(text: &str, model: TokenModel) -> CoreResult<u32> {
    match model {
        TokenModel::Gpt4o | TokenModel::Claude => {
            let enc = cl100k_encoder();
            Ok(enc.encode_with_special_tokens(text).len() as u32)
        }
        TokenModel::GeminiApprox => {
            let enc = cl100k_encoder();
            let base = enc.encode_with_special_tokens(text).len() as u32;
            // +5% rounded up — keep the math in u64 to avoid f64 rounding
            // surprises on long inputs. ceil(base * 1.05) == ceil(base*105/100).
            let scaled = (u64::from(base) * 105).div_ceil(100);
            Ok(scaled as u32)
        }
        TokenModel::Llama3 => LLAMA_3.count(text),
        TokenModel::Qwen2_5 => QWEN_2_5.count(text),
        TokenModel::DeepSeek => DEEPSEEK.count(text),
        TokenModel::Mistral => MISTRAL.count(text),
    }
}

/// Legacy string-keyed wrapper. Preserved so existing orchestrator callers
/// keep working unchanged. Intentionally does *not* dispatch through
/// [`count`] / [`TokenModel::Gpt4o`]: it keeps the original `o200k_base`
/// backend so observable token counts in v0.2.0 snapshots stay
/// byte-identical. Unknown strings return [`CoreError::TokenizerUnavailable`].
pub fn count_by_name(model: &str, text: &str) -> CoreResult<u32> {
    let enc = legacy_encoder(model)?;
    Ok(enc.encode_with_special_tokens(text).len() as u32)
}

fn legacy_encoder(model: &str) -> CoreResult<&'static CoreBPE> {
    match model {
        "gpt-4o-mini" | "gpt-4o" | "gpt-4" => Ok(O200K_BASE.get_or_init(|| {
            tiktoken_rs::o200k_base().expect("o200k_base encoder must initialize")
        })),
        _ => Err(CoreError::TokenizerUnavailable(model.into())),
    }
}

fn cl100k_encoder() -> &'static CoreBPE {
    CL100K_BASE.get_or_init(|| {
        tiktoken_rs::cl100k_base().expect("cl100k_base encoder must initialize")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- legacy string API (v0.2.0 behaviour, must remain unchanged) -------

    #[test]
    fn counts_tokens_in_simple_string() {
        let n = count_by_name("gpt-4o-mini", "Hello, world!").unwrap();
        assert!((1..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn empty_string_is_zero_tokens() {
        let n = count_by_name("gpt-4o-mini", "").unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn count_is_deterministic_across_calls() {
        let a = count_by_name("gpt-4o-mini", "fn main() { println!(\"hi\") }").unwrap();
        let b = count_by_name("gpt-4o-mini", "fn main() { println!(\"hi\") }").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn unknown_model_errors() {
        let err = count_by_name("not-a-real-model", "hi").unwrap_err();
        assert!(matches!(err, CoreError::TokenizerUnavailable(_)));
    }

    #[test]
    fn legacy_string_api_still_works() {
        // Explicit smoke test that the wrapper continues to dispatch.
        let n = count_by_name("gpt-4o-mini", "Hello, world!").unwrap();
        assert!(n > 0, "legacy wrapper must produce a non-zero count");
    }

    #[test]
    fn unknown_string_in_legacy_api_errors() {
        let err = count_by_name("not-a-real-model", "hi").unwrap_err();
        assert!(matches!(err, CoreError::TokenizerUnavailable(_)));
    }

    // --- new typed TokenModel API ------------------------------------------

    #[test]
    fn gpt4o_via_typed_api_counts_simple_string() {
        let n = count("Hello, world!", TokenModel::Gpt4o).unwrap();
        assert!((1..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn claude_uses_same_encoder_as_gpt4o() {
        let input = "The quick brown fox jumps over the lazy dog. 1234567890.";
        let g = count(input, TokenModel::Gpt4o).unwrap();
        let c = count(input, TokenModel::Claude).unwrap();
        assert_eq!(g, c, "Claude and Gpt4o must share the cl100k encoder");
    }

    #[test]
    fn gemini_approx_is_5pct_higher() {
        // Long enough that ceil(n * 1.05) is observably > n.
        let input = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
                     sed do eiusmod tempor incididunt ut labore et dolore magna \
                     aliqua. Ut enim ad minim veniam, quis nostrud exercitation \
                     ullamco laboris nisi ut aliquip ex ea commodo consequat. \
                     Duis aute irure dolor in reprehenderit in voluptate velit \
                     esse cillum dolore eu fugiat nulla pariatur.";
        let base = count(input, TokenModel::Gpt4o).unwrap();
        assert!(base >= 40, "test corpus too short: got {base} tokens");
        let approx = count(input, TokenModel::GeminiApprox).unwrap();
        let expected = (u64::from(base) * 105).div_ceil(100) as u32;
        assert_eq!(approx, expected);
        assert!(approx > base, "Gemini approx must exceed base count");
    }

    // --- HF-backed tokenizers (vendored JSONs, lazy parse) ----------------

    const HF_MODELS: [TokenModel; 4] = [
        TokenModel::Llama3,
        TokenModel::Qwen2_5,
        TokenModel::DeepSeek,
        TokenModel::Mistral,
    ];

    #[test]
    fn llama3_counts_simple_string() {
        let n = count("Hello, world!", TokenModel::Llama3).unwrap();
        assert!((3..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn qwen2_5_counts_simple_string() {
        let n = count("Hello, world!", TokenModel::Qwen2_5).unwrap();
        assert!((3..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn deepseek_counts_simple_string() {
        let n = count("Hello, world!", TokenModel::DeepSeek).unwrap();
        assert!((3..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn mistral_counts_simple_string() {
        let n = count("Hello, world!", TokenModel::Mistral).unwrap();
        assert!((3..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn hf_tokenizers_are_deterministic() {
        let input = "fn main() { println!(\"hi\"); } // a comment";
        for model in HF_MODELS {
            let a = count(input, model).unwrap();
            let b = count(input, model).unwrap();
            assert_eq!(a, b, "non-deterministic count for {model:?}: {a} vs {b}");
        }
    }

    #[test]
    fn hf_models_disagree_on_some_input() {
        // A long sentence with mixed punctuation, numbers and Unicode is highly
        // unlikely to land on the same token count across four independently-
        // trained vocabularies. If all four agree we've almost certainly
        // mis-wired and are routing every model through the same encoder.
        let input = "The quick brown fox jumps over 12 lazy dogs — and 3.14 \
                     pies, costing $42.99 each. Café résumé naïve façade.";
        let counts: Vec<u32> = HF_MODELS.iter().map(|m| count(input, *m).unwrap()).collect();
        assert!(
            counts.iter().any(|c| *c != counts[0]),
            "all 4 HF models returned identical counts {counts:?} — vocabularies should differ"
        );
    }

    #[test]
    fn lazy_init_does_not_panic() {
        // Force a first-call parse for every HF model. If any vendored JSON
        // fails to parse, `get_or_init` will run the expect-bearing closure
        // and crash this test deterministically.
        for model in HF_MODELS {
            let _ = count("warm up the OnceLock", model).unwrap();
        }
    }

    #[test]
    fn token_model_wire_strings_are_stable() {
        use serde_json::to_string;
        assert_eq!(to_string(&TokenModel::Gpt4o).unwrap(),        "\"gpt4o\"");
        assert_eq!(to_string(&TokenModel::Claude).unwrap(),       "\"claude\"");
        assert_eq!(to_string(&TokenModel::Llama3).unwrap(),       "\"llama3\"");
        assert_eq!(to_string(&TokenModel::Qwen2_5).unwrap(),      "\"qwen2_5\"");
        assert_eq!(to_string(&TokenModel::DeepSeek).unwrap(),     "\"deepSeek\"");
        assert_eq!(to_string(&TokenModel::Mistral).unwrap(),      "\"mistral\"");
        assert_eq!(to_string(&TokenModel::GeminiApprox).unwrap(), "\"geminiApprox\"");
        // round-trip
        assert_eq!(serde_json::from_str::<TokenModel>("\"gpt4o\"").unwrap(),    TokenModel::Gpt4o);
        assert_eq!(serde_json::from_str::<TokenModel>("\"qwen2_5\"").unwrap(),  TokenModel::Qwen2_5);
    }

    // --- count_all (per-model batch) ---------------------------------------

    #[test]
    fn count_all_returns_seven_counts() {
        let input = "fn main() { println!(\"hello, world!\"); }";
        let counts = count_all(input).unwrap();
        // All seven fields should be non-zero on a non-trivial input.
        assert!(counts.gpt4o > 0, "gpt4o = 0");
        assert!(counts.claude > 0, "claude = 0");
        assert!(counts.llama3 > 0, "llama3 = 0");
        assert!(counts.qwen2_5 > 0, "qwen2_5 = 0");
        assert!(counts.deep_seek > 0, "deep_seek = 0");
        assert!(counts.mistral > 0, "mistral = 0");
        assert!(counts.gemini_approx > 0, "gemini_approx = 0");
    }

    #[test]
    fn count_all_gpt4o_equals_claude() {
        // Both routes share the cl100k encoder, so the counts must match
        // byte-for-byte regardless of input.
        let input = "The quick brown fox jumps over the lazy dog. 1234567890.";
        let counts = count_all(input).unwrap();
        assert_eq!(counts.gpt4o, counts.claude);
    }

    #[test]
    fn count_all_gemini_is_gpt4o_times_105_ceil() {
        // Mirror the single-model `gemini_approx_is_5pct_higher` test: the
        // batched API must produce the same +5%-ceil math.
        let input = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
                     sed do eiusmod tempor incididunt ut labore et dolore magna \
                     aliqua. Ut enim ad minim veniam, quis nostrud exercitation \
                     ullamco laboris nisi ut aliquip ex ea commodo consequat.";
        let counts = count_all(input).unwrap();
        assert!(counts.gpt4o >= 40, "test corpus too short: got {} tokens", counts.gpt4o);
        let expected = (u64::from(counts.gpt4o) * 105).div_ceil(100) as u32;
        assert_eq!(counts.gemini_approx, expected);
        assert!(counts.gemini_approx > counts.gpt4o);
    }

    #[test]
    fn count_all_hf_models_disagree_with_each_other() {
        // Mixed punctuation / numerics / Unicode: extremely unlikely that
        // four independently-trained vocabularies all converge on the same
        // length. Catches accidental misrouting where every HF model would
        // funnel through the same encoder.
        let input = "The quick brown fox jumps over 12 lazy dogs — and 3.14 \
                     pies, costing $42.99 each. Café résumé naïve façade.";
        let counts = count_all(input).unwrap();
        let hf = [counts.llama3, counts.qwen2_5, counts.deep_seek, counts.mistral];
        assert!(
            hf.iter().any(|c| *c != hf[0]),
            "all 4 HF models returned identical counts {hf:?} — vocabularies should differ"
        );
    }
}
