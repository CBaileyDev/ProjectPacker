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

    out.push_str("# Repository Pack\n\n");

    out.push_str("## Stats\n\n");
    out.push_str("| Metric | Value |\n|--------|-------|\n");
    let _ = writeln!(out, "| Target | `{}` |", block.target_label);
    if !block.goal.is_empty() {
        let _ = writeln!(out, "| Goal | {} |", block.goal);
    }
    let _ = writeln!(
        out,
        "| Files | {} included \\| {} total \\| {} skipped |",
        block.files_included, block.files_total, block.files_skipped
    );
    let _ = writeln!(out, "| Bytes | {} |", block.bytes_total);
    if let Some(t) = block.tokens_total {
        let _ = writeln!(
            out,
            "| Tokens ({}) | {t} |",
            block.tokenizer_model
        );
    }
    if !block.languages.is_empty() {
        let _ = writeln!(out, "| Languages | {} |", block.languages_display());
    }
    let _ = writeln!(out, "| Redactions | {} |", block.redacted_bytes);
    let _ = writeln!(out, "| Cache hits | {} |", block.cache_hits);
    let _ = write!(out, "| Duration | {}ms |\n\n", block.duration_ms);

    // Security report (between stats and entries; emitted only when non-empty).
    let sec = security_report::emit_markdown(redactions);
    if !sec.is_empty() {
        out.push_str(&sec);
    }

    // Build the ordered slice: pinned entries in incoming order, then non-pinned
    // entries sorted alphabetically by path for diffability.
    let pinned_count = pinned_count.min(entries.len());
    let pinned = &entries[..pinned_count];
    let mut non_pinned: Vec<&FileEntry> = entries[pinned_count..].iter().collect();
    non_pinned.sort_by(|a, b| a.path.cmp(&b.path));

    out.push_str("## Directory Structure\n\n```\n");
    for e in pinned {
        out.push_str(&e.path);
        out.push('\n');
    }
    for e in &non_pinned {
        out.push_str(&e.path);
        out.push('\n');
    }
    out.push_str("```\n\n## Files\n\n");

    for e in pinned {
        let _ = write!(out, "### `{}`\n\n", e.path);
        let lang = ext_fence_lang(&e.path);
        let _ = writeln!(out, "```{lang}");
        out.push_str(&e.content);
        if !e.content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }
    for e in &non_pinned {
        let _ = write!(out, "### `{}`\n\n", e.path);
        let lang = ext_fence_lang(&e.path);
        let _ = writeln!(out, "```{lang}");
        out.push_str(&e.content);
        if !e.content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }

    out
}

fn ext_fence_lang(path: &str) -> &'static str {
    crate::lang::detect_from_path(path).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackFormat;

    fn opts() -> PackOptions {
        PackOptions {
            goal: "add a feature".into(),
            tokenizer_model: "gpt-4o-mini".into(),
            format: PackFormat::Markdown,
            ..PackOptions::default()
        }
    }

    fn stats() -> PackStats {
        PackStats {
            files_total: 2,
            files_included: 2,
            files_skipped: 0,
            bytes_total: 100,
            tokens_total: Some(50),
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 10,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: Some(50),
            emit_ms: 0,
            transforms: Vec::new(),
            transform_phase_ms: 0,
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
    fn renders_header_and_summary() {
        let out = render("my-repo", &opts(), &stats(), &[], 0, &[]);
        assert!(out.contains("# Repository Pack"));
        assert!(out.contains("`my-repo`"));
        assert!(out.contains("add a feature"));
        // files_included=2, files_total=2, files_skipped=0
        assert!(out.contains("2 included"));
        assert!(out.contains("gpt-4o-mini"));
        assert!(out.contains("| 50 |"));
    }

    #[test]
    fn renders_rust_file_with_correct_fence() {
        let entries = vec![FileEntry {
            path: "src/main.rs".into(),
            content: "fn main() {}\n".into(),
            bytes: 13,
            tokens: None,
            hash: "abc".into(),
        }];
        let out = render("repo", &opts(), &stats(), &entries, 0, &[]);
        assert!(out.contains("### `src/main.rs`"));
        assert!(out.contains("```rust\n"));
        assert!(out.contains("fn main() {}"));
    }

    #[test]
    fn renders_directory_structure_section() {
        let entries = vec![
            FileEntry { path: "a.rs".into(), content: "".into(), bytes: 0, tokens: None, hash: "".into() },
            FileEntry { path: "b.py".into(), content: "".into(), bytes: 0, tokens: None, hash: "".into() },
        ];
        let out = render("repo", &opts(), &stats(), &entries, 0, &[]);
        assert!(out.contains("## Directory Structure"));
        assert!(out.contains("a.rs"));
        assert!(out.contains("b.py"));
    }

    #[test]
    fn appends_trailing_newline_to_content_that_lacks_one() {
        let entries = vec![FileEntry {
            path: "x.ts".into(),
            content: "export {}".into(),
            bytes: 9,
            tokens: None,
            hash: "".into(),
        }];
        let out = render("r", &opts(), &stats(), &entries, 0, &[]);
        assert!(out.contains("export {}\n```"));
    }

    // Test 7: ## Stats replaces ## Summary.
    #[test]
    fn markdown_stats_section_replaces_summary() {
        let out = render("my-repo", &opts(), &stats(), &[], 0, &[]);
        assert!(out.contains("## Stats"), "output should contain ## Stats");
        assert!(
            !out.contains("## Summary"),
            "output must NOT contain ## Summary"
        );
        // Also verify key stats fields are present (and we haven't accidentally emitted XML tags).
        assert!(!out.contains("<pack_target>"), "MD output must not contain XML tags");
        assert!(out.contains("| Target |"));
        assert!(out.contains("| Redactions |"));
        assert!(out.contains("| Cache hits |"));
        // Empty redactions slice → row reads "| Redactions | 0 |".
        assert!(out.contains("| Redactions | 0 |"));
    }

    /// Markdown stats row reflects the redactions slice length, not a constant.
    #[test]
    fn markdown_redactions_row_reflects_slice_length() {
        let redactions = vec![
            PackRedaction {
                file: "a.rs".into(),
                rule_id: "aws-access-token".into(),
                line: 1,
                byte_offset: 10,
            },
            PackRedaction {
                file: "a.rs".into(),
                rule_id: "github-pat".into(),
                line: 4,
                byte_offset: 80,
            },
        ];
        let out = render("repo", &opts(), &stats(), &[], 0, &redactions);
        assert!(out.contains("| Redactions | 2 |"));
    }

    // ── Task F2 tests ─────────────────────────────────────────────────────────

    /// F2-6: Non-pinned entries are sorted alphabetically when pinned_count=0.
    #[test]
    fn markdown_alphabetizes_non_pinned_tail() {
        let entries = vec![entry("b.rs"), entry("a.rs")];
        let out = render("repo", &opts(), &stats(), &entries, 0, &[]);
        let a_pos = out.find("a.rs").expect("a.rs not in output");
        let b_pos = out.find("b.rs").expect("b.rs not in output");
        assert!(a_pos < b_pos, "a.rs must appear before b.rs after alphabetical sort");
    }

    /// F2-7: Pinned block stays in incoming order; non-pinned tail is sorted.
    #[test]
    fn markdown_keeps_pinned_block_in_incoming_order() {
        // Pinned: AGENTS.md, CLAUDE.md (declaration order)
        // Non-pinned: z.rs, a.rs — should be sorted to a.rs, z.rs
        let entries = vec![
            entry("AGENTS.md"),
            entry("CLAUDE.md"),
            entry("z.rs"),
            entry("a.rs"),
        ];
        let out = render("repo", &opts(), &stats(), &entries, 2, &[]);

        let agents_pos = out.find("AGENTS.md").expect("AGENTS.md not in output");
        let claude_pos = out.find("CLAUDE.md").expect("CLAUDE.md not in output");
        let a_pos = out.find("a.rs").expect("a.rs not in output");
        let z_pos = out.find("z.rs").expect("z.rs not in output");

        assert!(agents_pos < claude_pos, "AGENTS.md must come before CLAUDE.md (pinned order)");
        assert!(claude_pos < a_pos, "pinned block must come before non-pinned");
        assert!(a_pos < z_pos, "a.rs must come before z.rs (alphabetical sort)");
    }
}
