use crate::pack::FileEntry;
use crate::types::{PackOptions, PackRedaction, PackStats};

/// A rich stats block computed from PackStats + entries + options.
/// Emitted at the very top of every pack output to prime the LLM
/// with situational awareness about the pack.
pub struct StatsBlock {
    pub target_label: String,
    pub goal: String,
    pub files_included: u32,
    pub files_total: u32,
    pub files_skipped: u32,
    pub bytes_total: u64,
    pub tokens_total: Option<u32>,
    pub tokenizer_model: String,
    /// Top 5 languages by file count, sorted descending; ties broken alphabetically.
    pub languages: Vec<(String, u32)>,
    /// Number of secret redactions applied during the pack pipeline.
    ///
    /// The pack content shipped `[REDACTED:<rule-id>]` markers in place of the
    /// original secrets. Reporting `redactions.len()` (count) is more honest
    /// than "bytes saved" since (a) we don't track original secret length
    /// post-substitution and (b) the LLM cares about how many redactions
    /// happened, not their byte cost. The field name is retained for wire-
    /// format stability; the user-visible label in textual emitters reads
    /// "Redactions".
    pub redacted_bytes: u64,
    /// Phase 3 placeholder (content-addressed cache) — always 0 today.
    pub cache_hits: u32,
    pub duration_ms: u32,
}

impl StatsBlock {
    /// Compute a StatsBlock from PackStats + entries + options + redactions.
    pub fn from(
        target_label: &str,
        opts: &PackOptions,
        stats: &PackStats,
        entries: &[FileEntry],
        redactions: &[PackRedaction],
    ) -> Self {
        let languages = compute_language_breakdown(entries);
        Self {
            target_label: target_label.to_string(),
            goal: opts.goal.clone(),
            files_included: stats.files_included,
            files_total: stats.files_total,
            files_skipped: stats.files_skipped,
            bytes_total: stats.bytes_total,
            tokens_total: stats.tokens_total,
            tokenizer_model: opts.tokenizer_model.clone(),
            languages,
            // Semantically a redaction *count*; field name retained for wire-
            // format stability (renaming would churn TS bindings + the
            // emitted XML tag without functional benefit).
            redacted_bytes: redactions.len() as u64,
            cache_hits: 0,     // Phase 3 (content-addressed cache) will populate this
            duration_ms: stats.duration_ms,
        }
    }

    /// Format the languages vec as `rust: 8  typescript: 4  json: 2`.
    pub fn languages_display(&self) -> String {
        self.languages
            .iter()
            .map(|(lang, count)| format!("{lang}: {count}"))
            .collect::<Vec<_>>()
            .join("  ")
    }
}

/// Count files by detected language, take top 5, sorted descending by count
/// with alphabetical tie-breaking. Uses the shared `crate::lang::detect`
/// so this map stays in sync with the markdown emitter's fence-language map.
fn compute_language_breakdown(entries: &[FileEntry]) -> Vec<(String, u32)> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for entry in entries {
        let lang = crate::lang::detect_from_path(&entry.path)
            .map(str::to_string)
            .unwrap_or_else(|| "other".to_string());
        *counts.entry(lang).or_insert(0) += 1;
    }

    let mut sorted: Vec<(String, u32)> = counts.into_iter().collect();
    // Sort descending by count, ties broken alphabetically (ascending).
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.truncate(5);
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackFormat;

    fn make_entry(path: &str) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            content: String::new(),
            bytes: 0,
            tokens: None,
            hash: String::new(),
        }
    }

    fn make_opts() -> PackOptions {
        PackOptions {
            goal: String::new(),
            count_tokens: false,
            format: PackFormat::Xml,
            ..PackOptions::default()
        }
    }

    fn make_stats() -> PackStats {
        PackStats {
            files_total: 0,
            files_included: 0,
            files_skipped: 0,
            bytes_total: 0,
            tokens_total: None,
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 0,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
            transforms: Vec::new(),
            transform_phase_ms: 0,
        }
    }

    // Test 1: Basic extension-to-language counting.
    #[test]
    fn language_breakdown_counts_by_extension() {
        let entries = vec![
            make_entry("a.rs"),
            make_entry("b.rs"),
            make_entry("c.py"),
            make_entry("d.py"),
            make_entry("e.py"),
            make_entry("f.json"),
        ];
        let block = StatsBlock::from("target", &make_opts(), &make_stats(), &entries, &[]);
        // Expected: python: 3, rust: 2, json: 1
        assert_eq!(block.languages[0], ("python".to_string(), 3));
        assert_eq!(block.languages[1], ("rust".to_string(), 2));
        assert_eq!(block.languages[2], ("json".to_string(), 1));
    }

    // Test 2: Only top 5 languages are returned when more than 5 exist.
    #[test]
    fn language_breakdown_takes_top_5() {
        let entries = vec![
            make_entry("a.rs"),
            make_entry("b.py"),
            make_entry("c.js"),
            make_entry("d.ts"),
            make_entry("e.go"),
            make_entry("f.java"),
            make_entry("g.rb"),
        ];
        let block = StatsBlock::from("target", &make_opts(), &make_stats(), &entries, &[]);
        assert_eq!(block.languages.len(), 5);
    }

    // Test 3: Unknown extensions are grouped under "other".
    #[test]
    fn language_breakdown_groups_unknown_as_other() {
        let entries = vec![
            make_entry("a.weird"),
            make_entry("b.weird"),
            make_entry("c.strange"),
        ];
        let block = StatsBlock::from("target", &make_opts(), &make_stats(), &entries, &[]);
        // All unknowns go to "other"
        assert_eq!(block.languages.len(), 1);
        assert_eq!(block.languages[0].0, "other");
        assert_eq!(block.languages[0].1, 3);
    }

    // Test 4: Empty entries produces empty languages.
    #[test]
    fn language_breakdown_empty_when_no_entries() {
        let block = StatsBlock::from("target", &make_opts(), &make_stats(), &[], &[]);
        assert!(block.languages.is_empty());
    }

    // Test 6: redactions slice length is reflected in `redacted_bytes` field.
    // (Field name is retained for wire-format stability; semantics is "count".)
    #[test]
    fn redactions_slice_populates_redacted_bytes_count() {
        let redactions = vec![
            PackRedaction {
                file: "a.rs".into(),
                rule_id: "aws-access-token".into(),
                line: 12,
                byte_offset: 100,
            },
            PackRedaction {
                file: "b.rs".into(),
                rule_id: "github-pat".into(),
                line: 4,
                byte_offset: 50,
            },
            PackRedaction {
                file: "b.rs".into(),
                rule_id: "github-pat".into(),
                line: 9,
                byte_offset: 220,
            },
        ];
        let block = StatsBlock::from("target", &make_opts(), &make_stats(), &[], &redactions);
        assert_eq!(block.redacted_bytes, 3);

        // Empty slice → 0.
        let block_empty = StatsBlock::from("target", &make_opts(), &make_stats(), &[], &[]);
        assert_eq!(block_empty.redacted_bytes, 0);
    }

    // Test 5: Ties are broken alphabetically (ascending).
    #[test]
    fn language_breakdown_ties_break_alphabetically() {
        // rust: 3, python: 3 — should sort as python first (alphabetical)
        let entries = vec![
            make_entry("a.rs"),
            make_entry("b.rs"),
            make_entry("c.rs"),
            make_entry("d.py"),
            make_entry("e.py"),
            make_entry("f.py"),
        ];
        let block = StatsBlock::from("target", &make_opts(), &make_stats(), &entries, &[]);
        assert_eq!(block.languages.len(), 2);
        assert_eq!(block.languages[0].0, "python");
        assert_eq!(block.languages[1].0, "rust");
    }
}
