//! Token counting for various LLM tokenizers.
//!
//! Two public APIs live here:
//! - [`count`] (legacy, string-keyed) — preserved for backwards compatibility
//!   with existing callers in the orchestrator. Uses `o200k_base` for
//!   `"gpt-4o-mini"` / `"gpt-4o"` / `"gpt-4"` to keep observable token counts
//!   identical to v0.2.0.
//! - [`count_typed`] (new in v0.3.0, dispatched via [`TokenModel`]) — the
//!   forward path that frontends will move to. Uses `cl100k_base` for the
//!   OpenAI / Claude / Gemini-approx variants per the Phase 2 plan; HF-backed
//!   models return [`CoreError::TokenizerUnavailable`] until T2 wires them.

use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

// Two distinct encoder caches: legacy callers must keep the o200k counts they
// snapshot-tested in v0.2.0, while the new `TokenModel` API uses cl100k.
static O200K_BASE: OnceLock<CoreBPE> = OnceLock::new();
static CL100K_BASE: OnceLock<CoreBPE> = OnceLock::new();

/// Tokenizer family selector for the typed API.
///
/// Wire mapping (Phase 2 T1):
/// - `Gpt4o`, `Claude` → `cl100k_base` (Anthropic's tokenizer behaves close
///   enough to cl100k that we share the encoder).
/// - `GeminiApprox` → `cl100k_base` count multiplied by 1.05 and rounded up;
///   Gemini ships no public tokenizer so this is a deliberate over-estimate.
/// - `Llama3`, `Qwen2_5`, `DeepSeek`, `Mistral` → currently return
///   [`CoreError::TokenizerUnavailable`]; HF JSON tokenizers land in T2.
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

/// Count tokens for `text` using the encoder family selected by `model`.
pub fn count_typed(text: &str, model: TokenModel) -> CoreResult<u32> {
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
        // TODO(phase-2-t2): wire HF tokenizers
        TokenModel::Llama3 | TokenModel::Qwen2_5 | TokenModel::DeepSeek | TokenModel::Mistral => {
            Err(CoreError::TokenizerUnavailable(
                "model not yet wired".into(),
            ))
        }
    }
}

/// Legacy string-keyed wrapper. Preserved so existing orchestrator callers
/// keep working unchanged. Intentionally does *not* dispatch through
/// [`count_typed`] / [`TokenModel::Gpt4o`]: it keeps the original
/// `o200k_base` backend so observable token counts in v0.2.0 snapshots stay
/// byte-identical. Unknown strings return [`CoreError::TokenizerUnavailable`].
pub fn count(model: &str, text: &str) -> CoreResult<u32> {
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
        let n = count("gpt-4o-mini", "Hello, world!").unwrap();
        assert!((1..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn empty_string_is_zero_tokens() {
        let n = count("gpt-4o-mini", "").unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn count_is_deterministic_across_calls() {
        let a = count("gpt-4o-mini", "fn main() { println!(\"hi\") }").unwrap();
        let b = count("gpt-4o-mini", "fn main() { println!(\"hi\") }").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn unknown_model_errors() {
        let err = count("not-a-real-model", "hi").unwrap_err();
        assert!(matches!(err, CoreError::TokenizerUnavailable(_)));
    }

    #[test]
    fn legacy_string_api_still_works() {
        // Explicit smoke test that the wrapper continues to dispatch.
        let n = count("gpt-4o-mini", "Hello, world!").unwrap();
        assert!(n > 0, "legacy wrapper must produce a non-zero count");
    }

    #[test]
    fn unknown_string_in_legacy_api_errors() {
        let err = count("not-a-real-model", "hi").unwrap_err();
        assert!(matches!(err, CoreError::TokenizerUnavailable(_)));
    }

    // --- new typed TokenModel API ------------------------------------------

    #[test]
    fn gpt4o_via_typed_api_counts_simple_string() {
        let n = count_typed("Hello, world!", TokenModel::Gpt4o).unwrap();
        assert!((1..10).contains(&n), "got {n} tokens");
    }

    #[test]
    fn claude_uses_same_encoder_as_gpt4o() {
        let input = "The quick brown fox jumps over the lazy dog. 1234567890.";
        let g = count_typed(input, TokenModel::Gpt4o).unwrap();
        let c = count_typed(input, TokenModel::Claude).unwrap();
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
        let base = count_typed(input, TokenModel::Gpt4o).unwrap();
        assert!(base >= 40, "test corpus too short: got {base} tokens");
        let approx = count_typed(input, TokenModel::GeminiApprox).unwrap();
        let expected = (u64::from(base) * 105).div_ceil(100) as u32;
        assert_eq!(approx, expected);
        assert!(approx > base, "Gemini approx must exceed base count");
    }

    #[test]
    fn hf_models_return_tokenizer_unavailable() {
        for model in [
            TokenModel::Llama3,
            TokenModel::Qwen2_5,
            TokenModel::DeepSeek,
            TokenModel::Mistral,
        ] {
            let err = count_typed("hello", model).unwrap_err();
            assert!(
                matches!(err, CoreError::TokenizerUnavailable(_)),
                "expected TokenizerUnavailable for {model:?}, got {err:?}"
            );
        }
    }
}
