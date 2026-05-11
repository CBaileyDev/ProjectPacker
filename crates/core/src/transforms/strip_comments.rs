//! Thin wrapper around tree_sitter_compress::remove_comments.

use crate::tree_sitter_compress;

pub fn apply(path: &str, content: &str) -> Option<String> {
    let lang = tree_sitter_compress::detect_language(path)?;
    let new = tree_sitter_compress::remove_comments(content, lang);
    if new == content { None } else { Some(new) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_unknown_language() {
        assert!(apply("data.csv", "a,b,c\n").is_none());
    }

    #[test]
    fn strips_rust_comments() {
        let src = "// hello\nfn x() {}\n";
        let out = apply("a.rs", src).expect("should strip");
        assert!(!out.contains("hello"));
        assert!(out.contains("fn x"));
    }
}
