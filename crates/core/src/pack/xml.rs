use crate::pack::security_report;
use crate::pack::stats::StatsBlock;
use crate::pack::FileEntry;
use crate::types::{PackOptions, PackRedaction, PackStats};
use std::fmt::Write;

pub struct XmlBuilder {
    out: String,
}

impl Default for XmlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlBuilder {
    pub fn new() -> Self {
        Self { out: String::new() }
    }

    pub fn open_repository(&mut self) -> &mut Self {
        self.out.push_str("<repository>\n");
        self
    }

    pub fn close_repository(&mut self) -> &mut Self {
        self.out.push_str("</repository>\n");
        self
    }

    pub fn raw_block(&mut self, body: &str) -> &mut Self {
        self.out.push_str(body);
        if !body.ends_with('\n') {
            self.out.push('\n');
        }
        self
    }

    /// Emit a rich `<stats>` block at the top of the pack output.
    /// Replaces the old `<file_summary>` block.
    pub fn stats_block(
        &mut self,
        target_label: &str,
        opts: &PackOptions,
        stats: &PackStats,
        entries: &[FileEntry],
    ) -> &mut Self {
        let block = StatsBlock::from(target_label, opts, stats, entries);
        let _ = writeln!(self.out, "<stats>");
        let _ = writeln!(
            self.out,
            "  <pack_target>{}</pack_target>",
            escape_text(&block.target_label)
        );
        if !block.goal.is_empty() {
            let _ = writeln!(
                self.out,
                "  <goal>{}</goal>",
                escape_text(&block.goal)
            );
        }
        let _ = writeln!(
            self.out,
            "  <files>included={} total={} skipped={}</files>",
            block.files_included, block.files_total, block.files_skipped
        );
        let _ = writeln!(self.out, "  <bytes>{}</bytes>", block.bytes_total);
        if let Some(t) = block.tokens_total {
            let _ = writeln!(
                self.out,
                "  <tokens model=\"{}\">{t}</tokens>",
                escape_attr(&block.tokenizer_model)
            );
        }
        if !block.languages.is_empty() {
            let _ = writeln!(
                self.out,
                "  <languages>{}</languages>",
                block.languages_display()
            );
        }
        let _ = writeln!(
            self.out,
            "  <redacted_bytes>{}</redacted_bytes>",
            block.redacted_bytes
        );
        let _ = writeln!(
            self.out,
            "  <cache_hits>{}</cache_hits>",
            block.cache_hits
        );
        let _ = writeln!(
            self.out,
            "  <duration_ms>{}</duration_ms>",
            block.duration_ms
        );
        let _ = writeln!(self.out, "</stats>");
        self
    }

    /// Emit a `<security_report>` block listing every redaction performed
    /// during the pack pipeline. Empty input is a no-op so the byte-for-byte
    /// output is preserved when no secrets were found.
    pub fn security_report_block(&mut self, redactions: &[PackRedaction]) -> &mut Self {
        let fragment = security_report::emit_xml(redactions);
        if !fragment.is_empty() {
            self.out.push_str(&fragment);
        }
        self
    }

    pub fn directory_structure(&mut self, paths: &[String]) -> &mut Self {
        self.out.push_str("<directory_structure>\n");
        for p in paths {
            self.out.push_str(p);
            self.out.push('\n');
        }
        self.out.push_str("</directory_structure>\n");
        self
    }

    /// Emit the Anthropic cxml `<documents>` schema (default).
    ///
    /// The orchestrator's output is the canonical tail-priority ordering:
    /// pinned entries first (in declaration order), then non-pinned (walk order).
    /// Phase 5 (Map Mode) will substitute relevance-based ordering for the
    /// non-pinned segment — this emitter leaves the ordering it receives intact.
    ///
    /// Schema:
    /// ```xml
    /// <documents>
    ///   <document index="1">
    ///     <source>path/to/file</source>
    ///     <tokens>42</tokens>        <!-- omitted when None -->
    ///     <hash>abc123</hash>
    ///     <document_content>…</document_content>
    ///   </document>
    /// </documents>
    /// ```
    pub fn documents(&mut self, files: &[FileEntry]) -> &mut Self {
        self.out.push_str("<documents>\n");
        for (idx, f) in files.iter().enumerate() {
            let _ = writeln!(self.out, "<document index=\"{}\">", idx + 1);
            let _ = writeln!(self.out, "  <source>{}</source>", escape_text(&f.path));
            if let Some(t) = f.tokens {
                let _ = writeln!(self.out, "  <tokens>{t}</tokens>");
            }
            let _ = writeln!(self.out, "  <hash>{}</hash>", escape_text(&f.hash));
            self.out.push_str("  <document_content>");
            self.out.push_str(&escape_text(&f.content));
            if !f.content.ends_with('\n') {
                self.out.push('\n');
            }
            self.out.push_str("  </document_content>\n");
            self.out.push_str("</document>\n");
        }
        self.out.push_str("</documents>\n");
        self
    }

    /// Emit the legacy `<files>/<file>` schema.
    ///
    /// Called by the orchestrator when `xml_legacy_schema` is true (or
    /// `XmlSchema::Legacy`). Retained for backwards compatibility only.
    pub(crate) fn files_legacy(&mut self, files: &[FileEntry]) -> &mut Self {
        self.out.push_str("<files>\n");
        for f in files {
            let tokens_attr = match f.tokens {
                Some(t) => format!(" tokens=\"{t}\""),
                None => String::new(),
            };
            let _ = writeln!(
                self.out,
                "<file path=\"{}\"{tokens_attr} hash=\"{}\">",
                escape_attr(&f.path),
                f.hash
            );
            self.out.push_str(&escape_text(&f.content));
            if !f.content.ends_with('\n') {
                self.out.push('\n');
            }
            self.out.push_str("</file>\n");
        }
        self.out.push_str("</files>\n");
        self
    }

    /// Legacy alias kept so existing tests that call `builder.files(…)` still compile.
    /// Delegates to `files_legacy`.
    #[cfg(test)]
    pub fn files(&mut self, files: &[FileEntry]) -> &mut Self {
        self.files_legacy(files)
    }

    pub fn git_logs(&mut self, body: &str) -> &mut Self {
        self.out.push_str("<git_logs>\n");
        self.out.push_str(&escape_text(body));
        if !body.ends_with('\n') {
            self.out.push('\n');
        }
        self.out.push_str("</git_logs>\n");
        self
    }

    pub fn finish(self) -> String {
        self.out
    }
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackStats;

    fn make_entry(path: &str, content: &str, tokens: Option<u32>) -> FileEntry {
        FileEntry {
            path: path.into(),
            content: content.into(),
            bytes: content.len() as u64,
            tokens,
            hash: "deadbeef".into(),
        }
    }

    #[test]
    fn empty_repository_brackets() {
        let mut b = XmlBuilder::new();
        b.open_repository().close_repository();
        let s = b.finish();
        assert!(s.starts_with("<repository>"));
        assert!(s.ends_with("</repository>\n"));
    }

    #[test]
    fn escapes_attribute_quotes() {
        let entry = FileEntry {
            path: r#"a"b.txt"#.into(),
            content: "hi".into(),
            bytes: 2,
            tokens: None,
            hash: "abc".into(),
        };
        let mut b = XmlBuilder::new();
        b.files(&[entry]);
        let s = b.finish();
        assert!(s.contains(r#"path="a&quot;b.txt""#));
    }

    #[test]
    fn escapes_text_content_lt_gt_amp() {
        let entry = FileEntry {
            path: "a.txt".into(),
            content: "<x> & </x>".into(),
            bytes: 11,
            tokens: None,
            hash: "abc".into(),
        };
        let mut b = XmlBuilder::new();
        b.files(&[entry]);
        let s = b.finish();
        assert!(s.contains("&lt;x&gt; &amp; &lt;/x&gt;"));
    }

    // Test 6: stats block appears first inside <repository>.
    #[test]
    fn xml_stats_block_appears_first() {
        use crate::types::PackFormat;
        let opts = PackOptions {
            goal: "test".into(),
            count_tokens: true,
            tokenizer_model: "gpt-4o-mini".into(),
            format: PackFormat::Xml,
            ..PackOptions::default()
        };
        let stats = PackStats {
            files_total: 2,
            files_included: 1,
            files_skipped: 1,
            bytes_total: 500,
            tokens_total: Some(100),
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 42,
        };
        let entries = vec![FileEntry {
            path: "a.rs".into(),
            content: "fn main() {}".into(),
            bytes: 12,
            tokens: Some(5),
            hash: "abc".into(),
        }];

        let mut b = XmlBuilder::new();
        b.open_repository()
            .stats_block("my-target", &opts, &stats, &entries)
            .close_repository();
        let s = b.finish();

        // The doc opens with <repository> and immediately the <stats> block follows.
        assert!(s.starts_with("<repository>\n<stats>"));
        assert!(s.contains("<pack_target>my-target</pack_target>"));
        assert!(s.contains("<goal>test</goal>"));
        assert!(s.contains("<files>included=1 total=2 skipped=1</files>"));
        assert!(s.contains("<tokens model=\"gpt-4o-mini\">100</tokens>"));
        assert!(s.contains("</stats>"));
    }

    // ── Task F1 tests ─────────────────────────────────────────────────────────

    /// F1-1: `documents()` emits Anthropic cxml schema tags; legacy tags absent.
    #[test]
    fn documents_block_uses_anthropic_cxml_schema() {
        let entries = vec![
            make_entry("src/main.rs", "fn main() {}\n", Some(3)),
            make_entry("src/lib.rs", "pub fn foo() {}\n", None),
        ];
        let mut b = XmlBuilder::new();
        b.documents(&entries);
        let s = b.finish();

        assert!(s.contains("<documents>"), "must contain <documents>");
        assert!(s.contains("<document index=\"1\">"), "must contain index=1");
        assert!(s.contains("<source>src/main.rs</source>"), "must contain <source>");
        assert!(s.contains("<document_content>"), "must contain <document_content>");
        assert!(s.contains("<document index=\"2\">"), "must contain index=2");
        // Legacy tags must NOT appear.
        assert!(!s.contains("<files>"), "must NOT contain legacy <files>");
        assert!(!s.contains("<file path="), "must NOT contain legacy <file path=");
    }

    /// F1-2: index attribute is 1-based and monotonically increasing.
    #[test]
    fn documents_index_attribute_is_one_based_and_monotonic() {
        let entries = vec![
            make_entry("a.rs", "a\n", None),
            make_entry("b.rs", "b\n", None),
            make_entry("c.rs", "c\n", None),
        ];
        let mut b = XmlBuilder::new();
        b.documents(&entries);
        let s = b.finish();

        let pos1 = s.find("index=\"1\"").expect("index=1 missing");
        let pos2 = s.find("index=\"2\"").expect("index=2 missing");
        let pos3 = s.find("index=\"3\"").expect("index=3 missing");
        assert!(pos1 < pos2, "index=1 must come before index=2");
        assert!(pos2 < pos3, "index=2 must come before index=3");
    }

    /// F1-3: `<tokens>` element emitted only when tokens is Some.
    #[test]
    fn documents_emits_tokens_only_when_present() {
        let entries = vec![
            make_entry("a.rs", "fn main() {}\n", Some(5)),
            make_entry("b.rs", "fn foo() {}\n", None),
        ];
        let mut b = XmlBuilder::new();
        b.documents(&entries);
        let s = b.finish();

        // doc 1 must have <tokens>5</tokens>
        assert!(s.contains("<tokens>5</tokens>"), "doc 1 must contain <tokens>5</tokens>");

        // doc 2 must NOT have any <tokens> child element.
        // Find the second document block and check within it.
        let doc2_start = s.find("index=\"2\"").expect("index=2 missing");
        let doc2_end = s.find("</document>").and_then(|p| {
            // find the second </document>
            s[p + 1..].find("</document>").map(|q| p + 1 + q)
        }).unwrap_or(s.len());
        let doc2_text = &s[doc2_start..doc2_end];
        assert!(
            !doc2_text.contains("<tokens>"),
            "doc 2 must NOT contain <tokens> element, got: {doc2_text}"
        );
    }

    /// F1-4: XML special characters in content are escaped inside `<document_content>`.
    #[test]
    fn documents_escapes_xml_special_chars_in_content() {
        let entries = vec![make_entry("a.txt", "a < b & c\n", None)];
        let mut b = XmlBuilder::new();
        b.documents(&entries);
        let s = b.finish();
        assert!(
            s.contains("a &lt; b &amp; c"),
            "content must be XML-escaped, got: {s}"
        );
    }

    /// F1-5: legacy schema path emits `<files>` and `<file path=` when called.
    #[test]
    fn legacy_schema_path_emits_files_when_xml_legacy_schema_true() {
        let entries = vec![make_entry("src/main.rs", "fn main() {}\n", Some(3))];
        let mut b = XmlBuilder::new();
        b.files_legacy(&entries);
        let s = b.finish();
        assert!(s.contains("<files>"), "legacy must contain <files>");
        assert!(s.contains("<file path="), "legacy must contain <file path=");
    }
}
