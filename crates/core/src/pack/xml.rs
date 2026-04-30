use crate::pack::FileEntry;
use crate::types::PackStats;
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

    pub fn file_summary(&mut self, stats: &PackStats) -> &mut Self {
        let _ = writeln!(self.out, "<file_summary>");
        let _ = writeln!(self.out, "  files_total: {}", stats.files_total);
        let _ = writeln!(self.out, "  files_included: {}", stats.files_included);
        let _ = writeln!(self.out, "  files_skipped: {}", stats.files_skipped);
        let _ = writeln!(self.out, "  bytes_total: {}", stats.bytes_total);
        if let Some(t) = stats.tokens_total {
            let _ = writeln!(self.out, "  tokens_total: {t}");
        }
        let _ = writeln!(self.out, "  secrets_found: {}", stats.secrets_found);
        let _ = writeln!(self.out, "</file_summary>");
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

    pub fn files(&mut self, files: &[FileEntry]) -> &mut Self {
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

    fn empty_stats() -> PackStats {
        PackStats {
            files_total: 0,
            files_included: 0,
            files_skipped: 0,
            bytes_total: 0,
            tokens_total: None,
            secrets_found: 0,
            duration_ms: 0,
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

    #[test]
    fn file_summary_emits_stats_lines() {
        let mut b = XmlBuilder::new();
        let stats = PackStats {
            files_total: 5,
            files_included: 4,
            files_skipped: 1,
            bytes_total: 1024,
            tokens_total: Some(200),
            secrets_found: 0,
            duration_ms: 100,
        };
        b.file_summary(&stats);
        let s = b.finish();
        assert!(s.contains("files_total: 5"));
        assert!(s.contains("tokens_total: 200"));
    }

    #[test]
    fn unused_helper_ref_is_ok() {
        // ensure empty_stats() helper is referenced somewhere to avoid dead-code warnings
        let _ = empty_stats();
    }
}
