use crate::pack::security_report;
use crate::pack::stats::StatsBlock;
use crate::pack::FileEntry;
use crate::types::{PackOptions, PackRedaction, PackStats};
use std::fmt::Write;

pub fn render(
    root_label: &str,
    opts: &PackOptions,
    stats: &PackStats,
    entries: &[FileEntry],
    pinned_count: usize,
    redactions: &[PackRedaction],
) -> String {
    let block = StatsBlock::from(root_label, opts, stats, entries, redactions);
    let mut out = String::with_capacity((stats.bytes_total as usize).saturating_mul(2));

    out.push_str("=== STATS ===\n");
    let _ = writeln!(out, "Target: {}", block.target_label);
    if !block.goal.is_empty() {
        let _ = writeln!(out, "Goal: {}", block.goal);
    }
    let _ = writeln!(
        out,
        "Files: {} included | {} total | {} skipped",
        block.files_included, block.files_total, block.files_skipped
    );
    let _ = writeln!(out, "Bytes: {}", block.bytes_total);
    if let Some(t) = block.tokens_total {
        let _ = writeln!(
            out,
            "Tokens: {t} ({})",
            block.tokenizer_model
        );
    }
    if !block.languages.is_empty() {
        let _ = writeln!(out, "Languages: {}", block.languages_display());
    }
    // Field name on StatsBlock is `redacted_bytes` (kept for wire-format
    // stability), but it semantically holds the redaction *count*; emit it
    // as "Redactions" in the user-visible label.
    let _ = writeln!(out, "Redactions: {}", block.redacted_bytes);
    // Phase 3 (content-addressed cache) will populate cache_hits
    let _ = writeln!(out, "Cache hits: {}", block.cache_hits);
    let _ = writeln!(out, "Duration: {}ms", block.duration_ms);
    out.push_str("=== END STATS ===\n\n");

    // Security report (between stats and entries; emitted only when non-empty).
    let sec = security_report::emit_plain(redactions);
    if !sec.is_empty() {
        out.push_str(&sec);
        out.push('\n');
    }

    // Pinned entries in incoming order, then non-pinned sorted alphabetically.
    let pinned_count = pinned_count.min(entries.len());
    let pinned = &entries[..pinned_count];
    let mut non_pinned: Vec<&FileEntry> = entries[pinned_count..].iter().collect();
    non_pinned.sort_by(|a, b| a.path.cmp(&b.path));

    for e in pinned {
        let _ = writeln!(out, "=== {} ===", e.path);
        out.push_str(&e.content);
        if !e.content.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    for e in &non_pinned {
        let _ = writeln!(out, "=== {} ===", e.path);
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
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 5,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: Some(20),
            emit_ms: 0,
        }
    }

    fn entry(path: &str) -> FileEntry {
        FileEntry {
            path: path.into(),
            content: format!("// {path}\n"),
            bytes: path.len() as u64,
            tokens: None,
            hash: "".into(),
        }
    }

    #[test]
    fn renders_header_and_goal() {
        let out = render("my-repo", &opts(), &stats(), &[], 0, &[]);
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
        let out = render("repo", &opts(), &stats(), &entries, 0, &[]);
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
        let out = render("r", &opts(), &stats(), &entries, 0, &[]);
        assert!(out.contains("no newline\n"));
    }

    #[test]
    fn omits_token_line_when_count_tokens_false() {
        let mut o = opts();
        let mut s = stats();
        o.count_tokens = false;
        s.tokens_total = None;
        let out = render("r", &o, &s, &[], 0, &[]);
        assert!(!out.contains("Tokens:"));
    }

    // Test 8: Stats block uses === STATS === / === END STATS === delimiters.
    #[test]
    fn plain_stats_block_uses_delimiters() {
        let out = render("my-repo", &opts(), &stats(), &[], 0, &[]);
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
        // Empty redactions slice → "Redactions: 0".
        assert!(out.contains("Redactions: 0"));
        assert!(!out.contains("Redacted bytes:"));
    }

    /// Plain stats line reflects the redactions slice length.
    #[test]
    fn plain_redactions_line_reflects_slice_length() {
        let redactions = vec![PackRedaction {
            file: "a.rs".into(),
            rule_id: "aws-access-token".into(),
            line: 1,
            byte_offset: 10,
        }];
        let out = render("repo", &opts(), &stats(), &[], 0, &redactions);
        assert!(out.contains("Redactions: 1"));
    }

    // ── Task F2 tests ─────────────────────────────────────────────────────────

    /// F2-8: Non-pinned entries are sorted alphabetically when pinned_count=0.
    #[test]
    fn plain_alphabetizes_non_pinned_tail() {
        let entries = vec![entry("b.rs"), entry("a.rs")];
        let out = render("repo", &opts(), &stats(), &entries, 0, &[]);
        let a_pos = out.find("a.rs").expect("a.rs not in output");
        let b_pos = out.find("b.rs").expect("b.rs not in output");
        assert!(a_pos < b_pos, "a.rs must appear before b.rs after alphabetical sort");
    }
}
