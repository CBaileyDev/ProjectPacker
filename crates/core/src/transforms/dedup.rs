//! Cross-file deduplication via BLAKE3 hash grouping.

use crate::pack::FileEntry;
use crate::types::TransformReport;
use std::collections::HashMap;
use std::time::Instant;

/// For each group of files sharing a `hash`, the lexicographically-first path
/// keeps content; the rest get replaced with a marker referencing the first.
/// Mutates `entries` in place. Returns the report.
pub fn apply(entries: &mut [FileEntry]) -> TransformReport {
    let start = Instant::now();
    // Group indices by hash.
    let mut by_hash: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        // Skip empty hashes (failed reads).
        if e.hash.is_empty() { continue; }
        by_hash.entry(e.hash.as_str()).or_default().push(i);
    }
    // Collect groups with >1 member; lift index refs out before mutating.
    let dup_groups: Vec<Vec<usize>> = by_hash
        .into_values()
        .filter_map(|idxs| if idxs.len() > 1 { Some(idxs) } else { None })
        .collect();
    let mut bytes_saved: u64 = 0;
    let mut files_touched: u32 = 0;
    for group in dup_groups {
        // Find the path-lexicographic-min in the group.
        let mut sorted = group.clone();
        sorted.sort_by(|&a, &b| entries[a].path.cmp(&entries[b].path));
        let first_idx = sorted[0];
        let first_path = entries[first_idx].path.clone();
        let first_hash_prefix: String = entries[first_idx].hash.chars().take(12).collect();
        for &idx in sorted.iter().skip(1) {
            // Marker replaces the content; bytes_saved is the original byte count we no longer emit.
            let original_len = entries[idx].content.len() as u64;
            entries[idx].content = format!(
                "[DUPLICATE OF: {first_path} | sha: {first_hash_prefix}]\n"
            );
            // bytes_saved: original minus the marker we now emit.
            let new_len = entries[idx].content.len() as u64;
            bytes_saved = bytes_saved.saturating_add(original_len.saturating_sub(new_len));
            files_touched += 1;
        }
    }
    TransformReport {
        id: "dedup_files".into(),
        bytes_saved,
        files_touched,
        elapsed_ms: start.elapsed().as_millis() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, content: &str, hash: &str) -> FileEntry {
        FileEntry {
            path: path.into(),
            content: content.into(),
            bytes: content.len() as u64,
            tokens: None,
            hash: hash.into(),
        }
    }

    #[test]
    fn two_identical_files_keep_first_replace_second() {
        let body = "Apache License 2.0\n... full license body ...\n";
        let mut entries = vec![
            entry("LICENSE", body, "ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890"),
            entry("vendor/copy/LICENSE", body, "ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890"),
        ];
        let report = apply(&mut entries);
        assert_eq!(report.id, "dedup_files");
        assert_eq!(report.files_touched, 1);
        assert!(report.bytes_saved > 0);
        assert_eq!(entries[0].content, body, "first occurrence preserved");
        assert!(entries[1].content.starts_with("[DUPLICATE OF: LICENSE"));
        assert!(entries[1].content.contains("sha: ab12cd34ef56"));
    }

    #[test]
    fn three_identical_files_keep_first_replace_others() {
        let body = "x".repeat(500);
        let mut entries = vec![
            entry("c.txt", &body, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
            entry("a.txt", &body, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
            entry("b.txt", &body, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
        ];
        let report = apply(&mut entries);
        assert_eq!(report.files_touched, 2);
        // a.txt is the lexicographic minimum and should retain its content.
        let a = entries.iter().find(|e| e.path == "a.txt").unwrap();
        assert_eq!(a.content, body);
        let b = entries.iter().find(|e| e.path == "b.txt").unwrap();
        assert!(b.content.starts_with("[DUPLICATE OF: a.txt"));
        let c = entries.iter().find(|e| e.path == "c.txt").unwrap();
        assert!(c.content.starts_with("[DUPLICATE OF: a.txt"));
    }

    #[test]
    fn singletons_left_alone() {
        let mut entries = vec![
            entry("a.txt", "foo", "1111111111111111111111111111111111111111111111111111111111111111"),
            entry("b.txt", "bar", "2222222222222222222222222222222222222222222222222222222222222222"),
        ];
        // Plan-typo fix: original test used `assert_eq!(entries, original)`
        // but FileEntry does not derive PartialEq (and adding it would be a
        // public-API change radiating to bindings). Assert per-field instead.
        let original_a_content = entries[0].content.clone();
        let original_b_content = entries[1].content.clone();
        let report = apply(&mut entries);
        assert_eq!(report.files_touched, 0);
        assert_eq!(report.bytes_saved, 0);
        assert_eq!(entries[0].content, original_a_content);
        assert_eq!(entries[1].content, original_b_content);
    }

    #[test]
    fn empty_hashes_skipped() {
        let mut entries = vec![
            entry("a.txt", "", ""),
            entry("b.txt", "", ""),
        ];
        let report = apply(&mut entries);
        // No dedup against empty hashes — both kept untouched.
        assert_eq!(report.files_touched, 0);
    }

    #[test]
    fn sort_stability_path_lex_min_wins_regardless_of_input_order() {
        let body = "same\n";
        let hash = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        // Walker emits zzz first, but aaa should win the sort.
        let mut entries = vec![
            entry("zzz.txt", body, hash),
            entry("aaa.txt", body, hash),
        ];
        apply(&mut entries);
        let aaa = entries.iter().find(|e| e.path == "aaa.txt").unwrap();
        let zzz = entries.iter().find(|e| e.path == "zzz.txt").unwrap();
        assert_eq!(aaa.content, body, "lex-min path wins regardless of walker order");
        assert!(zzz.content.starts_with("[DUPLICATE OF: aaa.txt"));
    }
}
