pub mod xml;

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
