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
    pub tokens_total: Option<u32>,
    pub secrets_found: u32,
    pub duration_ms: u32,
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
    BuildingXml,
    Done {
        stats: PackStats,
    },
    Error {
        message: String,
        fatal: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackResult {
    pub xml: String,
    pub claude_code_prompt: String,
    pub stats: PackStats,
    pub warnings: Vec<PackWarning>,
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
                secrets_found: 0,
                duration_ms: 200,
            },
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"kind\":\"done\""));
        assert!(s.contains("\"filesTotal\":10"));
    }
}
