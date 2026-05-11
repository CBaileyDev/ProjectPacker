use crate::pack::security_report;
use crate::pack::stats::StatsBlock;
use crate::pack::xml_escape_attr as escape_attr;
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

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            out: String::with_capacity(cap),
        }
    }

    /// Construct a builder pre-sized to `estimate` bytes. Sugar over
    /// [`Self::with_capacity`] paired with [`estimated_xml_capacity`] for
    /// callers that already know the entries' total byte count.
    pub fn with_capacity_estimate(estimate: usize) -> Self {
        Self::with_capacity(estimate)
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
        redactions: &[PackRedaction],
    ) -> &mut Self {
        let block = StatsBlock::from(target_label, opts, stats, entries, redactions);
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
        // Field name retained for wire-format stability; semantically a
        // redaction *count*. The visible tag stays `<redacted_bytes>` to
        // avoid churning downstream consumers (TS bindings + LLM prompts).
        let _ = writeln!(
            self.out,
            "  <redactions>{}</redactions>",
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

    /// Emit a `<compression_report>` block summarising every transform that
    /// ran during the transform phase. Empty input (no transforms enabled or
    /// the phase was skipped) is a no-op so the byte-for-byte output is
    /// preserved when nothing was applied.
    ///
    /// Schema:
    /// ```xml
    /// <compression_report>
    ///   <transform id="trim_trailing_ws" bytes_saved="12" files_touched="1" elapsed_ms="0"/>
    ///   …
    /// </compression_report>
    /// ```
    pub fn compression_report_block(&mut self, stats: &PackStats) -> &mut Self {
        if stats.transforms.is_empty() {
            return self;
        }
        self.out.push_str("<compression_report>\n");
        for r in &stats.transforms {
            let _ = writeln!(
                self.out,
                "  <transform id=\"{}\" bytes_saved=\"{}\" files_touched=\"{}\" elapsed_ms=\"{}\"/>",
                escape_attr(&r.id),
                r.bytes_saved,
                r.files_touched,
                r.elapsed_ms,
            );
        }
        self.out.push_str("</compression_report>\n");
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
            // Deduped entries (produced by the `dedup_files` transform) carry
            // a content marker of the shape:
            //     "[DUPLICATE OF: <path> | sha: <12-char-prefix>]\n"
            // Emit them as a self-closing `<document>` with `path` +
            // `duplicate-of` + `sha` attributes instead of a full-body element.
            // Falls back to the normal body emit if the marker fails to parse —
            // we never want to lose content because of a marker-shape change.
            if let Some((dup_path, dup_sha)) = parse_dup_marker(&f.content) {
                let _ = writeln!(
                    self.out,
                    "<document index=\"{}\" path=\"{}\" duplicate-of=\"{}\" sha=\"{}\"/>",
                    idx + 1,
                    escape_attr(&f.path),
                    escape_attr(dup_path),
                    escape_attr(dup_sha),
                );
                continue;
            }

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

    pub fn finish(self) -> String {
        self.out
    }
}

/// Estimate the output buffer size for a pack of XML-formatted
/// `entries`. Uses `1.2 * entries_total_bytes + 140 * num_entries +
/// 512`, all in saturating arithmetic so callers never overflow on
/// pathologically-large inputs. The 1.2x factor covers worst-case
/// `&amp;` / `&lt;` / `&gt;` escape expansion (3-4x growth on pure
/// special-character input is rare in practice; 1.2x is the
/// large-real-corpus average from the snapshot fixtures). The 140-byte
/// per-entry overhead is the wrapper tag mass for a `<document>` block;
/// the 512-byte fixed tail covers `<repository>` / `<stats>` envelopes.
pub fn estimated_xml_capacity(entries_total_bytes: u64, num_entries: usize) -> usize {
    let body = (entries_total_bytes.saturating_mul(12) / 10) as usize;
    let per_entry = num_entries.saturating_mul(140);
    body.saturating_add(per_entry).saturating_add(512)
}

/// Parse a `[DUPLICATE OF: <path> | sha: <prefix>]` content marker emitted
/// by the `dedup_files` transform. Returns `Some((path, sha_prefix))` on a
/// well-formed marker, `None` otherwise — callers must fall through to the
/// normal full-body emit on `None` so unrelated content that happens to
/// start with the prefix is never lost.
fn parse_dup_marker(content: &str) -> Option<(&str, &str)> {
    const DUP_PREFIX: &str = "[DUPLICATE OF: ";
    let rest = content.strip_prefix(DUP_PREFIX)?;
    let (path_part, after) = rest.split_once(" | sha: ")?;
    // The marker ends at the first `]`; tolerate either a trailing newline
    // or none.
    let sha_part = after.split(']').next()?;
    if sha_part.is_empty() {
        return None;
    }
    Some((path_part, sha_part))
}

/// Single-pass-allocation XML text escape. Replaces `&`, `<`, `>` with
/// their entities. Implemented as two scans — the first counts the
/// extra bytes the escaped output will need, the second writes the
/// result into a `String` pre-sized to exactly `s.len() + extra` bytes
/// — which avoids the chained-`replace` quadratic-realloc behaviour and
/// over-allocation of the previous implementation.
fn escape_text(s: &str) -> String {
    // Pass 1: count extra bytes needed.
    //   `&` → `&amp;`  (+4)
    //   `<` → `&lt;`   (+3)
    //   `>` → `&gt;`   (+3)
    let mut extra = 0usize;
    for &b in s.as_bytes() {
        match b {
            b'&' => extra += 4,
            b'<' | b'>' => extra += 3,
            _ => {}
        }
    }
    if extra == 0 {
        return s.to_owned();
    }
    // Pass 2: build the output exactly once.
    let mut out = String::with_capacity(s.len() + extra);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
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
        b.files_legacy(&[entry]);
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
        b.files_legacy(&[entry]);
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
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
            transforms: Vec::new(),
            transform_phase_ms: 0,
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
            .stats_block("my-target", &opts, &stats, &entries, &[])
            .close_repository();
        let s = b.finish();

        // The doc opens with <repository> and immediately the <stats> block follows.
        assert!(s.starts_with("<repository>\n<stats>"));
        assert!(s.contains("<pack_target>my-target</pack_target>"));
        assert!(s.contains("<goal>test</goal>"));
        assert!(s.contains("<files>included=1 total=2 skipped=1</files>"));
        assert!(s.contains("<tokens model=\"gpt-4o-mini\">100</tokens>"));
        // Empty redactions slice → "<redactions>0</redactions>".
        assert!(s.contains("<redactions>0</redactions>"));
        assert!(!s.contains("<redacted_bytes>"));
        assert!(s.contains("</stats>"));
    }

    /// XML stats `<redactions>` tag reflects the redactions slice length.
    #[test]
    fn xml_stats_block_redactions_tag_reflects_slice_length() {
        use crate::types::PackFormat;
        let opts = PackOptions {
            goal: "x".into(),
            format: PackFormat::Xml,
            ..PackOptions::default()
        };
        let stats = PackStats {
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
        };
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
                line: 5,
                byte_offset: 90,
            },
        ];
        let mut b = XmlBuilder::new();
        b.stats_block("t", &opts, &stats, &[], &redactions);
        let s = b.finish();
        assert!(s.contains("<redactions>2</redactions>"));
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

    // ── escape_text / estimated_xml_capacity tests ─────────────────────────

    #[test]
    fn escape_text_empty_input() {
        assert_eq!(escape_text(""), "");
    }

    #[test]
    fn escape_text_all_ascii_no_specials() {
        let s = "hello, world! 123 abc";
        // No specials → returns owned clone byte-for-byte identical.
        assert_eq!(escape_text(s), s);
    }

    #[test]
    fn escape_text_all_special_chars() {
        // Only `&`, `<`, `>` — every byte expands.
        let out = escape_text("&<>&<>");
        assert_eq!(out, "&amp;&lt;&gt;&amp;&lt;&gt;");
    }

    #[test]
    fn escape_text_mixed_content() {
        let out = escape_text("a < b && c > d");
        assert_eq!(out, "a &lt; b &amp;&amp; c &gt; d");
    }

    #[test]
    fn escape_text_capacity_matches_exact_output_length() {
        // The two-scan strategy promises `s.len() + extra` is the exact
        // capacity needed — verify by walking through the full
        // input/output pair.
        let s = "a&b<c>d";
        let out = escape_text(s);
        // Each special char expands to entity:
        //   & → &amp; (+4) | < → &lt; (+3) | > → &gt; (+3) → +10
        assert_eq!(out.len(), s.len() + 10);
        assert_eq!(out, "a&amp;b&lt;c&gt;d");
    }

    #[test]
    fn estimated_xml_capacity_zero_inputs() {
        // Empty input still yields the fixed 512-byte tail.
        assert_eq!(estimated_xml_capacity(0, 0), 512);
    }

    #[test]
    fn estimated_xml_capacity_typical_pack() {
        // 100 KiB across 50 entries → 1.2x body + 50*140 + 512.
        let bytes: u64 = 100 * 1024;
        let n: usize = 50;
        let expected = (bytes * 12 / 10) as usize + n * 140 + 512;
        assert_eq!(estimated_xml_capacity(bytes, n), expected);
    }

    #[test]
    fn estimated_xml_capacity_saturates_on_huge_input() {
        // u64::MAX bytes shouldn't overflow when scaled by 1.2x.
        let cap = estimated_xml_capacity(u64::MAX, usize::MAX);
        // Result must be a finite usize; the precise value is platform-
        // dependent, but it must equal usize::MAX (saturating).
        assert_eq!(cap, usize::MAX);
    }
}
