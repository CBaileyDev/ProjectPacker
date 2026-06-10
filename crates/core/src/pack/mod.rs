pub mod markdown;
pub mod orchestrator;
pub mod pin;
pub mod plain;
pub mod security_report;
pub mod stats;
pub mod xml;

pub use orchestrator::{pack, PackEvent};

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub content: String,
    pub bytes: u64,
    pub tokens: Option<u32>,
    pub hash: String,
}

/// Entry ordering shared by the markdown and plain renderers: the pinned
/// prefix (first `pinned_count` entries) stays in incoming (declaration)
/// order, the non-pinned tail is sorted alphabetically by path for
/// diffability. Returns references in that final render order.
pub(crate) fn pinned_then_sorted(entries: &[FileEntry], pinned_count: usize) -> Vec<&FileEntry> {
    let pinned_count = pinned_count.min(entries.len());
    let (pinned, tail) = entries.split_at(pinned_count);
    let mut non_pinned: Vec<&FileEntry> = tail.iter().collect();
    non_pinned.sort_by(|a, b| a.path.cmp(&b.path));
    let mut ordered: Vec<&FileEntry> = Vec::with_capacity(entries.len());
    ordered.extend(pinned.iter());
    ordered.extend(non_pinned);
    ordered
}

/// Escape a value for inclusion in a double-quoted XML attribute.
///
/// Escapes `&`, `<`, `>`, and `"` per XML 1.0 §2.4. Apostrophes are not
/// escaped because all attributes in our emitters use double-quote delimiters.
pub(crate) fn xml_escape_attr(s: &str) -> String {
    xml_escape_with(s, |b| match b {
        b'&' => Some("&amp;"),
        b'<' => Some("&lt;"),
        b'>' => Some("&gt;"),
        b'"' => Some("&quot;"),
        _ => None,
    })
}

/// Shared scanning escaper behind [`xml_escape_attr`] and the text escape in
/// `pack::xml`. Scans the byte stream for bytes that `rep` maps to an entity
/// and copies the unchanged spans between matches wholesale (`&str` slice
/// copies, not per-char pushes). When nothing needs escaping the input is
/// returned as a single plain copy with no per-char work.
///
/// Correctness: `rep` must only map single-byte ASCII values (`b < 0x80`).
/// Those bytes never occur inside a multibyte UTF-8 sequence, so slicing `s`
/// at match positions always lands on char boundaries and multibyte content
/// is copied through intact.
pub(crate) fn xml_escape_with(s: &str, rep: fn(u8) -> Option<&'static str>) -> String {
    let mut out = String::new();
    xml_escape_into(&mut out, s, rep);
    out
}

/// Appending form of [`xml_escape_with`]: escapes `s` straight into `out`
/// (reserving exactly the needed room first), so hot paths that already
/// hold a pre-sized output buffer skip the escaped-temporary allocation
/// and second copy.
pub(crate) fn xml_escape_into(out: &mut String, s: &str, rep: fn(u8) -> Option<&'static str>) {
    let bytes = s.as_bytes();
    // Pass 1: count extra bytes so pass 2 reserves exactly once.
    let mut extra = 0usize;
    for &b in bytes {
        if let Some(r) = rep(b) {
            extra += r.len() - 1;
        }
    }
    out.reserve(s.len() + extra);
    if extra == 0 {
        out.push_str(s);
        return;
    }
    // Pass 2: copy span-wise, splicing entities in at the matched bytes.
    let mut copied_to = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if let Some(r) = rep(b) {
            out.push_str(&s[copied_to..i]);
            out.push_str(r);
            copied_to = i + 1;
        }
    }
    out.push_str(&s[copied_to..]);
}

/// Output-buffer size estimate shared by the markdown and plain renderers:
/// content bytes plus a 2x margin for wrappers, tables, and escapes.
pub(crate) fn estimated_text_capacity(bytes_total: u64) -> usize {
    (bytes_total as usize).saturating_mul(2)
}
