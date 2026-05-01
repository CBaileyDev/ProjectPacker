use crate::pack::FileEntry;
use crate::types::{PackOptions, PackStats};

pub fn render(
    root_label: &str,
    opts: &PackOptions,
    stats: &PackStats,
    entries: &[FileEntry],
) -> String {
    let mut out = String::new();

    out.push_str("=== REPOSITORY PACK ===\n");
    out.push_str(&format!("Target: {root_label}\n"));
    if !opts.goal.is_empty() {
        out.push_str(&format!("Goal: {}\n", opts.goal));
    }
    out.push_str(&format!(
        "Files: {}/{} included  |  {} skipped  |  {} bytes",
        stats.files_included, stats.files_total, stats.files_skipped, stats.bytes_total
    ));
    if let Some(t) = stats.tokens_total {
        out.push_str(&format!("  |  {t} tokens ({})", opts.tokenizer_model));
    }
    out.push_str("\n\n");

    for e in entries {
        out.push_str(&format!("=== {} ===\n", e.path));
        out.push_str(&e.content);
        if !e.content.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackFormat;

    fn opts() -> PackOptions {
        PackOptions {
            goal: "test goal".into(),
            tokenizer_model: "gpt-4o-mini".into(),
            format: PackFormat::PlainText,
            ..PackOptions::default()
        }
    }

    fn stats() -> PackStats {
        PackStats {
            files_total: 1,
            files_included: 1,
            files_skipped: 0,
            bytes_total: 50,
            tokens_total: Some(20),
            secrets_found: 0,
            duration_ms: 5,
        }
    }

    #[test]
    fn renders_header_and_goal() {
        let out = render("my-repo", &opts(), &stats(), &[]);
        assert!(out.starts_with("=== REPOSITORY PACK ===\n"));
        assert!(out.contains("Target: my-repo"));
        assert!(out.contains("Goal: test goal"));
        assert!(out.contains("1/1 included"));
        assert!(out.contains("20 tokens"));
        assert!(out.contains("gpt-4o-mini"));
    }

    #[test]
    fn renders_file_section_with_separator() {
        let entries = vec![FileEntry {
            path: "src/lib.rs".into(),
            content: "pub fn foo() {}\n".into(),
            bytes: 16,
            tokens: None,
            hash: "abc".into(),
        }];
        let out = render("repo", &opts(), &stats(), &entries);
        assert!(out.contains("=== src/lib.rs ===\n"));
        assert!(out.contains("pub fn foo() {}"));
    }

    #[test]
    fn appends_newline_when_content_missing_trailing() {
        let entries = vec![FileEntry {
            path: "a.txt".into(),
            content: "no newline".into(),
            bytes: 10,
            tokens: None,
            hash: "".into(),
        }];
        let out = render("r", &opts(), &stats(), &entries);
        assert!(out.contains("no newline\n"));
    }

    #[test]
    fn omits_token_line_when_count_tokens_false() {
        let mut o = opts();
        let mut s = stats();
        o.count_tokens = false;
        s.tokens_total = None;
        let out = render("r", &o, &s, &[]);
        assert!(!out.contains("tokens"));
    }
}
