use crate::error::{CoreError, CoreResult};
use crate::ignore::IgnoreMatcher;
use crate::pack::xml::XmlBuilder;
use crate::pack::{markdown, plain};
use crate::pack::FileEntry;
use crate::types::PackFormat;
use crate::protocol;
use crate::secrets;
use crate::tokens;
use crate::tree_sitter_compress;
use crate::types::*;
use crate::walker::{self, WalkOptions};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Instant;

pub type PackEvent = ProgressEvent;

pub fn pack(
    root: &Path,
    opts: &PackOptions,
    tx: Sender<PackEvent>,
    job_id: &str,
) -> CoreResult<PackResult> {
    let start = Instant::now();
    let mut warnings: Vec<PackWarning> = Vec::new();

    let label = root.display().to_string();
    let _ = tx.send(ProgressEvent::Started {
        job_id: job_id.into(),
        target_label: label.clone(),
    });

    let matcher = IgnoreMatcher::new(root, &opts.custom_ignore_patterns, opts.respect_gitignore);
    let outcome = walker::walk(
        root,
        &matcher,
        &WalkOptions {
            max_file_size_kb: opts.max_file_size_kb,
        },
    );

    let _ = tx.send(ProgressEvent::Walking {
        files_scanned: outcome.included.len() as u32,
    });
    let _ = tx.send(ProgressEvent::FileFoundBatch {
        paths: outcome.included.clone(),
    });

    for (p, r) in &outcome.skipped {
        let _ = tx.send(ProgressEvent::FileSkipped {
            path: p.clone(),
            reason: r.clone(),
        });
    }

    let _ = tx.send(ProgressEvent::BuildingXml);

    let results: Vec<(FileEntry, Vec<PackWarning>)> = outcome
        .included
        .par_iter()
        .map(|f| {
            let abs = root.join(&f.path);
            let mut file_warnings: Vec<PackWarning> = Vec::new();

            let raw = match read_text_with_fallback(&abs) {
                Ok((content, fallback)) => {
                    if fallback {
                        file_warnings.push(PackWarning {
                            kind: WarningKind::EncodingFallback,
                            path: Some(f.path.clone()),
                            message: "Decoded as non-UTF-8 (UTF-16 or Windows-1252)".into(),
                        });
                    }
                    content
                }
                Err(e) => {
                    file_warnings.push(PackWarning {
                        kind: WarningKind::FileSkipped,
                        path: Some(f.path.clone()),
                        message: format!("Read failed: {e}"),
                    });
                    String::new()
                }
            };

            // Step 1: strip comments if requested (tree-sitter languages only).
            let after_comments = if opts.remove_comments {
                if let Some(lang) = tree_sitter_compress::detect_language(&f.path) {
                    tree_sitter_compress::remove_comments(&raw, lang)
                } else {
                    raw.clone()
                }
            } else {
                raw.clone()
            };

            // Step 2: optionally compress to a skeleton.
            let content = if opts.compress {
                if let Some(lang) = tree_sitter_compress::detect_language(&f.path) {
                    tree_sitter_compress::compress(&after_comments, lang)
                } else {
                    after_comments.clone()
                }
            } else {
                after_comments
            };

            let tokens = if opts.count_tokens {
                tokens::count(&opts.tokenizer_model, &content).ok()
            } else {
                None
            };

            let mut hasher = Sha256::new();
            hasher.update(raw.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            (
                FileEntry {
                    path: f.path.clone(),
                    content,
                    bytes: f.bytes,
                    tokens,
                    hash,
                },
                file_warnings,
            )
        })
        .collect();

    let mut entries: Vec<FileEntry> = Vec::with_capacity(results.len());
    for (entry, w) in results {
        entries.push(entry);
        warnings.extend(w);
    }

    let mut secrets_found = 0u32;
    if opts.secret_scan {
        for e in &entries {
            for hit in secrets::scan(&e.content) {
                secrets_found += 1;
                let _ = tx.send(ProgressEvent::SecretHit {
                    path: e.path.clone(),
                    secret_kind: hit.kind,
                    line: hit.line,
                });
            }
        }
    }

    let mut bytes_total = 0u64;
    let mut tokens_total: u32 = 0;
    for e in &entries {
        bytes_total += e.bytes;
        if let Some(t) = e.tokens {
            tokens_total += t;
        }
    }

    let stats = PackStats {
        files_total: (outcome.included.len() + outcome.skipped.len()) as u32,
        files_included: entries.len() as u32,
        files_skipped: outcome.skipped.len() as u32,
        bytes_total,
        tokens_total: opts.count_tokens.then_some(tokens_total),
        secrets_found,
        duration_ms: start.elapsed().as_millis() as u32,
    };

    let dir_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();

    let output = match opts.format {
        PackFormat::Xml => {
            let protocol_block = protocol::block_for_pack(&opts.goal, &opts.protocol_version)?;
            let mut builder = XmlBuilder::new();
            builder
                .open_repository()
                .raw_block(&protocol_block)
                .file_summary(&stats)
                .directory_structure(&dir_paths)
                .files(&entries)
                .close_repository();
            builder.finish()
        }
        PackFormat::Markdown => markdown::render(&label, opts, &stats, &entries),
        PackFormat::PlainText => plain::render(&label, opts, &stats, &entries),
    };

    let claude_code_prompt = protocol::claude_code_prompt(&opts.protocol_version)?;

    let _ = tx.send(ProgressEvent::Done {
        stats: stats.clone(),
    });

    Ok(PackResult {
        output,
        claude_code_prompt,
        stats,
        warnings,
    })
}

/// Read a file as text, returning `(content, used_non_utf8_fallback)`.
///
/// Detection order:
///   1. UTF-8 (with optional BOM stripped)
///   2. UTF-16 LE (BOM `FF FE`)
///   3. UTF-16 BE (BOM `FE FF`)
///   4. Windows-1252 (final fallback; always succeeds)
fn read_text_with_fallback(path: &Path) -> CoreResult<(String, bool)> {
    let bytes = std::fs::read(path).map_err(|e| CoreError::FileIo {
        path: path.to_path_buf(),
        source: e,
    })?;

    // 1. UTF-8 with optional BOM
    let without_bom = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    if let Ok(s) = std::str::from_utf8(without_bom) {
        return Ok((s.to_string(), false));
    }

    // 2. UTF-16 LE BOM
    if bytes.starts_with(b"\xFF\xFE") {
        let (cow, _, _) = encoding_rs::UTF_16LE.decode(&bytes);
        return Ok((cow.into_owned(), true));
    }

    // 3. UTF-16 BE BOM
    if bytes.starts_with(b"\xFE\xFF") {
        let (cow, _, _) = encoding_rs::UTF_16BE.decode(&bytes);
        return Ok((cow.into_owned(), true));
    }

    // 4. Final fallback: Windows-1252 (always succeeds)
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
    Ok((cow.into_owned(), true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fixture() -> tempfile::TempDir {
        let d = tempdir().unwrap();
        fs::write(d.path().join("a.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
        fs::write(d.path().join("README.md"), "# title\n\nText.\n").unwrap();
        d
    }

    #[test]
    fn end_to_end_produces_xml_and_stats() {
        let d = fixture();
        let opts = PackOptions {
            goal: "Add a hello".into(),
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(d.path(), &opts, tx, "job-test").unwrap();
        assert!(result.output.contains("<protocol version=\"grok-to-cc-v1\">"));
        assert!(result.output.contains("<files>"));
        assert!(result.output.contains("README.md"));
        assert!(result.output.contains("a.rs"));
        assert_eq!(result.stats.files_included, 2);
        assert!(result
            .claude_code_prompt
            .contains("EXECUTOR with veto power"));
    }

    #[test]
    fn emits_progress_events_in_expected_order() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: false,
            secret_scan: false,
            ..PackOptions::default()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = pack(d.path(), &opts, tx, "job-test").unwrap();
        let mut events: Vec<&'static str> = Vec::new();
        for ev in rx.try_iter() {
            events.push(match ev {
                ProgressEvent::Started { .. } => "started",
                ProgressEvent::Walking { .. } => "walking",
                ProgressEvent::FileFoundBatch { .. } => "batch",
                ProgressEvent::FileSkipped { .. } => "skipped",
                ProgressEvent::BuildingXml => "building",
                ProgressEvent::Done { .. } => "done",
                _ => "other",
            });
        }
        assert_eq!(events.first(), Some(&"started"));
        assert_eq!(events.last(), Some(&"done"));
        assert!(events.contains(&"building"));
    }

    #[test]
    fn read_text_with_fallback_handles_plain_utf8() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.txt");
        fs::write(&p, "hello world").unwrap();
        let (s, fallback) = read_text_with_fallback(&p).unwrap();
        assert_eq!(s, "hello world");
        assert!(!fallback);
    }

    #[test]
    fn read_text_with_fallback_strips_utf8_bom() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.txt");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"hello");
        fs::write(&p, &bytes).unwrap();
        let (s, fallback) = read_text_with_fallback(&p).unwrap();
        assert_eq!(s, "hello");
        assert!(!fallback);
    }

    #[test]
    fn read_text_with_fallback_decodes_utf16_le() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.txt");
        // UTF-16 LE BOM + "hi"
        let bytes: [u8; 6] = [0xFF, 0xFE, 0x68, 0x00, 0x69, 0x00];
        fs::write(&p, bytes).unwrap();
        let (s, fallback) = read_text_with_fallback(&p).unwrap();
        assert_eq!(s, "hi");
        assert!(fallback);
    }

    #[test]
    fn read_text_with_fallback_decodes_utf16_be() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.txt");
        // UTF-16 BE BOM + "hi"
        let bytes: [u8; 6] = [0xFE, 0xFF, 0x00, 0x68, 0x00, 0x69];
        fs::write(&p, bytes).unwrap();
        let (s, fallback) = read_text_with_fallback(&p).unwrap();
        assert_eq!(s, "hi");
        assert!(fallback);
    }

    #[test]
    fn read_text_with_fallback_decodes_windows_1252() {
        let d = tempdir().unwrap();
        let p = d.path().join("a.txt");
        // 'h' + 'é' (0xE9 in Windows-1252; invalid as UTF-8 start byte)
        let bytes: [u8; 2] = [0x68, 0xE9];
        fs::write(&p, bytes).unwrap();
        let (s, fallback) = read_text_with_fallback(&p).unwrap();
        assert_eq!(s, "hé");
        assert!(fallback);
    }

    #[test]
    fn pack_emits_encoding_fallback_warning() {
        let d = tempdir().unwrap();
        // Write a Windows-1252 file (will trigger encoding fallback).
        // NOTE: original task spec used UTF-16 LE bytes [0xFF, 0xFE, 0x68, 0x00, 0x69, 0x00],
        // but the walker's `is_binary` check flags any file containing a 0x00 byte as binary
        // and skips it before the orchestrator ever calls read_text_with_fallback. Windows-1252
        // bytes with no nulls survive the walker and exercise the same EncodingFallback path.
        let bytes: [u8; 2] = [0x68, 0xE9]; // 'h' + 'é' (0xE9 invalid as UTF-8 start byte)
        fs::write(d.path().join("note.txt"), bytes).unwrap();

        let opts = PackOptions {
            goal: "x".into(),
            secret_scan: false,
            count_tokens: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(d.path(), &opts, tx, "job-test").unwrap();

        assert!(
            result
                .warnings
                .iter()
                .any(|w| matches!(w.kind, WarningKind::EncodingFallback)
                    && w.path.as_deref() == Some("note.txt")),
            "expected an EncodingFallback warning for note.txt, got {:?}",
            result.warnings
        );
    }

    #[test]
    fn read_text_with_fallback_errors_on_missing_file() {
        let result = read_text_with_fallback(std::path::Path::new(
            "/definitely/does/not/exist/xyz.txt",
        ));
        assert!(matches!(result, Err(CoreError::FileIo { .. })));
    }
}
