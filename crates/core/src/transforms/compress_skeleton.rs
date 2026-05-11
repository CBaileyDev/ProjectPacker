//! Thin wrapper around tree_sitter_compress::compress, gated by the
//! `compress` option. Returns Some(new) only when a language was detected
//! AND the content actually changed.

use crate::tree_sitter_compress;

pub fn apply(path: &str, content: &str) -> Option<String> {
    let lang = tree_sitter_compress::detect_language(path)?;
    let new = tree_sitter_compress::compress(content, lang);
    if new == content { None } else { Some(new) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_unknown_language() {
        assert!(apply("README.md", "# title\n").is_none());
    }

    #[test]
    fn compresses_rust_file() {
        let src = "fn add(a: i32, b: i32) -> i32 { a + b }\n";
        let out = apply("a.rs", src).expect("should compress");
        assert!(out.contains("fn add"));
        assert!(!out.contains("a + b"));
    }
}
