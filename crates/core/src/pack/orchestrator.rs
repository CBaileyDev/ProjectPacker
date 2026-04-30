use crate::error::{CoreError, CoreResult};
use crate::ignore::IgnoreMatcher;
use crate::pack::xml::XmlBuilder;
use crate::pack::FileEntry;
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
    let warnings: Vec<PackWarning> = Vec::new();

    let label = root.display().to_string();
    let _ = tx.send(ProgressEvent::Started {
        job_id: job_id.into(),
        target_label: label,
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

    let entries: Vec<FileEntry> = outcome
        .included
        .par_iter()
        .map(|f| {
            let abs = root.join(&f.path);
            let raw = read_text(&abs).unwrap_or_default();
            let content = if opts.compress {
                if let Some(lang) = tree_sitter_compress::detect_language(&f.path) {
                    tree_sitter_compress::compress(&raw, lang)
                } else {
                    raw.clone()
                }
            } else {
                raw.clone()
            };

            let tokens = if opts.count_tokens {
                tokens::count(&opts.tokenizer_model, &content).ok()
            } else {
                None
            };

            let mut hasher = Sha256::new();
            hasher.update(raw.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            FileEntry {
                path: f.path.clone(),
                content,
                bytes: f.bytes,
                tokens,
                hash,
            }
        })
        .collect();

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
    let protocol_block = protocol::block_for_pack(&opts.goal, &opts.protocol_version)?;
    let mut builder = XmlBuilder::new();
    builder
        .open_repository()
        .raw_block(&protocol_block)
        .file_summary(&stats)
        .directory_structure(&dir_paths)
        .files(&entries)
        .close_repository();
    let output = builder.finish();

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

fn read_text(path: &Path) -> CoreResult<String> {
    let bytes = std::fs::read(path).map_err(|e| CoreError::FileIo {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            let (cow, _, _) = encoding_rs::UTF_16LE.decode(&bytes);
            cow.into_owned()
        }
    })
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
}
