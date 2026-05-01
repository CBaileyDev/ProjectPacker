use crate::pack::stats::StatsBlock;
use crate::pack::FileEntry;
use crate::types::{PackOptions, PackStats};

pub fn render(
    root_label: &str,
    opts: &PackOptions,
    stats: &PackStats,
    entries: &[FileEntry],
) -> String {
    let block = StatsBlock::from(root_label, opts, stats, entries);
    let mut out = String::new();

    out.push_str("=== STATS ===\n");
    out.push_str(&format!("Target: {}\n", block.target_label));
    if !block.goal.is_empty() {
        out.push_str(&format!("Goal: {}\n", block.goal));
    }
    out.push_str(&format!(
        "Files: {} included | {} total | {} skipped\n",
        block.files_included, block.files_total, block.files_skipped
    ));
    out.push_str(&format!("Bytes: {}\n", block.bytes_total));
    if let Some(t) = block.tokens_total {
        out.push_str(&format!(
            "Tokens: {t} ({})\n",
            block.tokenizer_model
        ));
    }
    if !block.languages.is_empty() {
        out.push_str(&format!("Languages: {}\n", block.languages_display()));
    }
    // Phase 2 (secret redaction) will populate redacted_bytes
    out.push_str(&format!("Redacted bytes: {}\n", block.redacted_bytes));
    // Phase 3 (content-addressed cache) will populate cache_hits
    out.push_str(&format!("Cache hits: {}\n", block.cache_hits));
    out.push_str(&format!("Duration: {}ms\n", block.duration_ms));
    out.push_str("=== END STATS ===\n\n");

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
        assert!(out.starts_with("=== STATS ===\n"));
        assert!(out.contains("Target: my-repo"));
        assert!(out.contains("Goal: test goal"));
        assert!(out.contains("1 included | 1 total | 0 skipped"));
        assert!(out.contains("Tokens: 20 (gpt-4o-mini)"));
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
        assert!(!out.contains("Tokens:"));
    }

    // Test 8: Stats block uses === STATS === / === END STATS === delimiters.
    #[test]
    fn plain_stats_block_uses_delimiters() {
        let out = render("my-repo", &opts(), &stats(), &[]);
        assert!(
            out.contains("=== STATS ==="),
            "output should contain === STATS ==="
        );
        assert!(
            out.contains("=== END STATS ==="),
            "output should contain === END STATS ==="
        );
        // Verify it does NOT use the old REPOSITORY PACK header at the top
        assert!(!out.starts_with("=== REPOSITORY PACK ==="));
    }
}
