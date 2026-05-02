use crate::tokens::TokensPerModel;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "value")]
pub enum PackTarget {
    #[serde(rename = "folder")]
    Folder(PathBuf),
    #[serde(rename = "github")]
    GitHub(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum PackFormat {
    #[default]
    Xml,
    Markdown,
    PlainText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub enum XmlSchema {
    #[default]
    Cxml,   // Anthropic <documents> shape — new default
    Legacy, // <files><file path="..."> shape — kept for backwards compat
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackOptions {
    pub target: PackTarget,
    pub goal: String,
    pub include_git_history: bool,
    pub count_tokens: bool,
    pub tokenizer_model: String,
    pub secret_scan: bool,
    pub compress: bool,
    pub remove_comments: bool,
    pub max_file_size_kb: u32,
    pub respect_gitignore: bool,
    pub custom_ignore_patterns: Vec<String>,
    pub protocol_version: String,
    pub format: PackFormat,
    /// Defaulted via `serde(default)` so v0.1 settings missing this field
    /// deserialize cleanly to `XmlSchema::Cxml` (the new default).
    #[serde(default)]
    pub xml_schema: XmlSchema,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            target: PackTarget::Folder(PathBuf::from(".")),
            goal: String::new(),
            include_git_history: false,
            count_tokens: true,
            tokenizer_model: "gpt-4o-mini".into(),
            secret_scan: true,
            compress: false,
            remove_comments: false,
            max_file_size_kb: 1024,
            respect_gitignore: true,
            custom_ignore_patterns: Vec::new(),
            protocol_version: "grok-to-cc-v1".into(),
            format: PackFormat::Xml,
            xml_schema: XmlSchema::Cxml,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackStats {
    pub files_total: u32,
    pub files_included: u32,
    pub files_skipped: u32,
    pub bytes_total: u64,
    /// Token count under the user-selected tokenizer (`opts.tokenizer_model`),
    /// summed across all included files. `None` when `count_tokens` is off.
    pub tokens_total: Option<u32>,
    /// Per-model token counts of the joined pack content, computed via the
    /// authentic tokenizer of each model family (with cl100k as a proxy for
    /// Claude/Gemini, which don't ship public tokenizers). Surfaced in the
    /// AI compatibility table on the result screen. `None` when
    /// `count_tokens` is off, mirroring `tokens_total`.
    pub tokens_per_model: Option<TokensPerModel>,
    pub secrets_found: u32,
    pub duration_ms: u32,
    /// Per-phase wall-clock elapsed time. Always populated; `Option` variants
    /// are `None` when the phase is skipped via `PackOptions` (e.g.
    /// `secret_scan_ms` is `None` when `opts.secret_scan == false`). Use
    /// `None` (not `Some(0)`) so the UI can render skipped phases as `—`
    /// rather than misleading "0ms".
    pub walk_ms: u32,
    pub process_ms: u32,
    pub secret_scan_ms: Option<u32>,
    pub tokenize_ms: Option<u32>,
    pub emit_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkipReason {
    Ignored,
    TooLarge,
    Binary,
    Inaccessible,
    EncodingFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum WarningKind {
    FileSkipped,
    TreeSitterFailed,
    GitLogMissing,
    EncodingFallback,
    SecretScanFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackWarning {
    pub kind: WarningKind,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileFound {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ProgressEvent {
    Started {
        job_id: String,
        target_label: String,
    },
    Cloning {
        progress_pct: u8,
    },
    Walking {
        files_scanned: u32,
    },
    FileFoundBatch {
        paths: Vec<FileFound>,
    },
    FileSkipped {
        path: String,
        reason: SkipReason,
    },
    Tokenizing {
        progress_pct: u8,
    },
    SecretScanning {
        progress_pct: u8,
    },
    SecretHit {
        path: String,
        secret_kind: String,
        line: u32,
    },
    Compressing {
        progress_pct: u8,
    },
    BuildingOutput,
    Done {
        stats: PackStats,
    },
    Error {
        message: String,
        fatal: bool,
    },
}

/// A redaction performed during the pack pipeline, surfaced in the
/// `<security_report>` block and via `PackResult.redactions`.
///
/// Note: deliberately omits the matched-excerpt field on the underlying
/// [`crate::secrets::Redaction`] — the security report should not reproduce
/// secrets, even partially.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackRedaction {
    pub file: String,
    pub rule_id: String,
    pub line: u32,
    pub byte_offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackResult {
    pub output: String,
    pub claude_code_prompt: String,
    pub stats: PackStats,
    pub warnings: Vec<PackWarning>,
    pub redactions: Vec<PackRedaction>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_options_default_has_v1_protocol() {
        let opts = PackOptions::default();
        assert_eq!(opts.protocol_version, "grok-to-cc-v1");
        assert_eq!(opts.tokenizer_model, "gpt-4o-mini");
        assert_eq!(opts.max_file_size_kb, 1024);
        assert!(opts.respect_gitignore);
        assert_eq!(opts.format, PackFormat::Xml);
    }

    #[test]
    fn pack_target_round_trips_through_json_folder() {
        let t = PackTarget::Folder(PathBuf::from("/tmp/repo"));
        let s = serde_json::to_string(&t).unwrap();
        let back: PackTarget = serde_json::from_str(&s).unwrap();
        match back {
            PackTarget::Folder(p) => assert_eq!(p, PathBuf::from("/tmp/repo")),
            _ => panic!("expected Folder variant"),
        }
    }

    #[test]
    fn pack_target_round_trips_through_json_github() {
        let t = PackTarget::GitHub("https://github.com/user/repo".into());
        let s = serde_json::to_string(&t).unwrap();
        let back: PackTarget = serde_json::from_str(&s).unwrap();
        match back {
            PackTarget::GitHub(u) => assert_eq!(u, "https://github.com/user/repo"),
            _ => panic!("expected GitHub variant"),
        }
    }

    #[test]
    fn progress_event_done_serializes_with_stats() {
        let ev = ProgressEvent::Done {
            stats: PackStats {
                files_total: 10,
                files_included: 9,
                files_skipped: 1,
                bytes_total: 12345,
                tokens_total: Some(2000),
                tokens_per_model: None,
                secrets_found: 0,
                duration_ms: 200,
                walk_ms: 5,
                process_ms: 100,
                secret_scan_ms: Some(20),
                tokenize_ms: Some(50),
                emit_ms: 25,
            },
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"kind\":\"done\""));
        assert!(s.contains("\"filesTotal\":10"));
    }
}
