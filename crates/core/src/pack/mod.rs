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

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub content: String,
    pub bytes: u64,
    pub tokens: Option<u32>,
    pub hash: String,
}

/// Escape a value for inclusion in a double-quoted XML attribute.
///
/// Escapes `&`, `<`, `>`, and `"` per XML 1.0 §2.4. Apostrophes are not
/// escaped because all attributes in our emitters use double-quote delimiters.
pub(crate) fn xml_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
