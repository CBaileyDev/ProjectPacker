use crate::error::{CoreError, CoreResult};
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

static GPT4O_MINI: OnceLock<CoreBPE> = OnceLock::new();

pub fn count(model: &str, text: &str) -> CoreResult<u32> {
    let enc = encoder(model)?;
    Ok(enc.encode_with_special_tokens(text).len() as u32)
}

fn encoder(model: &str) -> CoreResult<&'static CoreBPE> {
    match model {
        "gpt-4o-mini" | "gpt-4o" | "gpt-4" => Ok(GPT4O_MINI.get_or_init(|| {
            tiktoken_rs::o200k_base().expect("o200k_base encoder must initialize")
        })),
        _ => Err(CoreError::TokenizerUnavailable(model.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
