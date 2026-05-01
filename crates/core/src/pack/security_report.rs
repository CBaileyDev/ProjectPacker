//! `<security_report>` block emitter.
//!
//! Produces a per-format fragment listing each redaction performed during
//! the pack pipeline. Embedded in the pack output immediately after the
//! stats block and before any file entries.
//!
//! Empty input → empty string (callers must skip injecting empty
//! fragments so byte-for-byte equivalence with the no-secrets case is
//! preserved).

use crate::types::PackRedaction;
use std::collections::HashSet;
use std::fmt::Write;

/// Emit the security report as an XML fragment for embedding in the cxml
/// output. Empty input returns `""` — callers must skip injection.
pub fn emit_xml(redactions: &[PackRedaction]) -> String {
    if redactions.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<security_report total_redactions=\"{}\">",
        redactions.len()
    );
    for r in redactions {
        let _ = writeln!(
            out,
            "  <redaction file=\"{}\" rule_id=\"{}\" line=\"{}\" byte_offset=\"{}\"/>",
            escape_attr(&r.file),
            escape_attr(&r.rule_id),
            r.line,
            r.byte_offset,
        );
    }
    out.push_str("</security_report>\n");
    out
}

/// Markdown variant. Empty input returns `""`.
pub fn emit_markdown(redactions: &[PackRedaction]) -> String {
    if redactions.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let unique_files: HashSet<&str> = redactions.iter().map(|r| r.file.as_str()).collect();
    out.push_str("## Security Report\n\n");
    let _ = writeln!(
        out,
        "{} redactions across {} files. Each match was replaced with `[REDACTED:<rule-id>]` in the pack content.\n",
        redactions.len(),
        unique_files.len(),
    );
    out.push_str("| Rule | File | Line | Byte Offset |\n");
    out.push_str("|------|------|------|-------------|\n");
    for r in redactions {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            r.rule_id, r.file, r.line, r.byte_offset,
        );
    }
    out.push('\n');
    out
}

/// Plain-text variant. Empty input returns `""`.
pub fn emit_plain(redactions: &[PackRedaction]) -> String {
    if redactions.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let unique_files: HashSet<&str> = redactions.iter().map(|r| r.file.as_str()).collect();
    out.push_str("=== SECURITY REPORT ===\n");
    let _ = writeln!(
        out,
        "{} redactions across {} files.",
        redactions.len(),
        unique_files.len(),
    );
    out.push_str("Each match was replaced with [REDACTED:<rule-id>] in the pack content.\n\n");

    // Compute padding widths for nicer alignment, capped to keep extreme
    // paths from blowing up the column.
    let rule_w = redactions
        .iter()
        .map(|r| r.rule_id.len())
        .max()
        .unwrap_or(0)
        .max(4);
    let path_w = redactions
        .iter()
        .map(|r| r.file.len() + 1 + r.line.to_string().len())
        .max()
        .unwrap_or(0)
        .max(4);

    for r in redactions {
        let path_line = format!("{}:{}", r.file, r.line);
        let _ = writeln!(
            out,
            "{:rule_w$}  {:path_w$}  (offset {})",
            r.rule_id,
            path_line,
            r.byte_offset,
            rule_w = rule_w,
            path_w = path_w,
        );
    }
    out.push_str("=== END SECURITY REPORT ===\n");
    out
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(file: &str, rule_id: &str, line: u32, byte_offset: u32) -> PackRedaction {
        PackRedaction {
            file: file.into(),
            rule_id: rule_id.into(),
            line,
            byte_offset,
        }
    }

    #[test]
    fn emit_xml_empty_returns_empty_string() {
        assert_eq!(emit_xml(&[]), "");
    }

    #[test]
    fn emit_markdown_empty_returns_empty_string() {
        assert_eq!(emit_markdown(&[]), "");
    }

    #[test]
    fn emit_plain_empty_returns_empty_string() {
        assert_eq!(emit_plain(&[]), "");
    }

    #[test]
    fn emit_xml_escapes_path_with_special_chars() {
        // Path with `&` and `<` should appear escaped in the XML output.
        let reds = vec![r("a&b<c.txt", "aws-access-token", 1, 0)];
        let out = emit_xml(&reds);
        assert!(
            out.contains("a&amp;b&lt;c.txt"),
            "expected escaped path, got: {out}"
        );
        assert!(!out.contains("a&b<c.txt"), "raw path leaked: {out}");
        assert!(out.contains("<security_report total_redactions=\"1\">"));
    }

    #[test]
    fn emit_xml_includes_total_redactions_attribute() {
        let reds = vec![
            r("a.txt", "aws-access-token", 1, 0),
            r("b.txt", "github-pat", 2, 5),
        ];
        let out = emit_xml(&reds);
        assert!(out.contains("total_redactions=\"2\""));
        assert!(out.contains("</security_report>"));
    }

    #[test]
    fn emit_markdown_lists_all_redactions() {
        let reds = vec![
            r("src/danger.txt", "aws-access-token", 2, 6),
            r("config/api.toml", "github-pat", 10, 42),
            r("config/api.toml", "generic-api-key", 15, 80),
        ];
        let out = emit_markdown(&reds);
        assert!(out.contains("## Security Report"));
        assert!(out.contains("aws-access-token"), "missing aws rule id");
        assert!(out.contains("github-pat"), "missing github rule id");
        assert!(out.contains("generic-api-key"), "missing generic rule id");
        assert!(out.contains("3 redactions across 2 files"));
        assert!(out.contains("| Rule | File | Line | Byte Offset |"));
    }

    #[test]
    fn emit_plain_includes_offset_label() {
        let reds = vec![r("src/danger.txt", "aws-access-token", 2, 6)];
        let out = emit_plain(&reds);
        assert!(out.contains("=== SECURITY REPORT ==="));
        assert!(out.contains("(offset 6)"), "missing (offset N) label: {out}");
        assert!(out.contains("aws-access-token"));
        assert!(out.contains("src/danger.txt:2"));
    }
}
