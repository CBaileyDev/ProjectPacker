//! Minified-bundle detection + body collapse.

const LONG_LINE_THRESHOLD: usize = 2000;
const AVG_OVER_MEDIAN_RATIO: f64 = 5.0;

pub fn is_minified(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() { return false; }
    let has_long = lines.iter().any(|l| l.len() > LONG_LINE_THRESHOLD);
    if !has_long { return false; }
    // Single-line files: long line alone is sufficient.
    if lines.len() <= 5 { return true; }
    // Multi-line: check avg/median ratio to suppress tab-aligned data files.
    let mut lens: Vec<usize> = lines.iter().map(|l| l.len()).collect();
    lens.sort_unstable();
    let median = lens[lens.len() / 2].max(1);
    let avg = (lens.iter().sum::<usize>() as f64) / (lens.len() as f64);
    (avg / median as f64) > AVG_OVER_MEDIAN_RATIO
}

/// Returns `Some(collapsed)` if `content` looks minified, else `None`.
pub fn collapse(content: &str, hash_prefix: &str) -> Option<String> {
    if !is_minified(content) { return None; }
    let n = content.len();
    let head: String = content.chars().take(200).collect();
    let tail: String = content.chars().rev().take(100).collect::<String>()
        .chars().rev().collect();
    Some(format!(
        "{head}\n[MINIFIED BUNDLE: {n} bytes | sha: {hash_prefix}]\n{tail}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_single_line_minified_bundle() {
        let content: String = "a=1;".repeat(1000); // ~4000 chars, 1 line
        assert!(is_minified(&content));
    }

    #[test]
    fn does_not_detect_normal_source() {
        let content = "fn main() {\n    println!(\"hi\");\n}\n".repeat(20);
        assert!(!is_minified(&content));
    }

    #[test]
    fn does_not_detect_data_file_with_consistent_long_lines() {
        // 100 lines each 2100 chars — long, but consistent length.
        let line = "x".repeat(2100);
        let content = (0..100).map(|_| line.clone()).collect::<Vec<_>>().join("\n");
        // avg ≈ median, ratio ≈ 1, should NOT trip the heuristic.
        assert!(!is_minified(&content));
    }

    #[test]
    fn detects_mixed_short_lines_plus_one_huge_line() {
        // 50 short lines (10 chars) + 1 huge line (5000 chars).
        // avg = (50*10 + 5000) / 51 ≈ 108; median = 10; ratio ≈ 11 → trip.
        let mut content = String::new();
        for _ in 0..50 { content.push_str("short line\n"); }
        content.push_str(&"x".repeat(5000));
        content.push('\n');
        assert!(is_minified(&content));
    }

    #[test]
    fn collapse_replaces_minified_body() {
        let content: String = "var x=".repeat(500); // ~3000 chars, 1 line, no \n
        let out = collapse(&content, "abc123def456").unwrap();
        assert!(out.contains("[MINIFIED BUNDLE:"));
        assert!(out.contains("sha: abc123def456"));
        assert!(out.contains(&content[..200])); // head preserved
    }

    #[test]
    fn collapse_returns_none_for_non_minified() {
        let content = "regular\nsource\ncode\n";
        assert!(collapse(content, "abc").is_none());
    }
}
