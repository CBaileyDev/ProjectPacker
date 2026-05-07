use crate::error::{CoreError, CoreResult};
use crate::ignore::IgnoreMatcher;
use crate::pack::pin;
use crate::pack::xml::XmlBuilder;
use crate::pack::{markdown, plain};
use crate::pack::FileEntry;
use crate::protocol;
use crate::secrets;
use crate::tokens;
use crate::tree_sitter_compress;
use crate::types::{
    FileFound, PackFormat, PackOptions, PackRedaction, PackResult, PackStats,
    PackTarget, PackWarning, ProgressEvent, WarningKind, XmlSchema,
};
use crate::walker::{self, WalkOptions};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

pub type PackEvent = ProgressEvent;

pub fn pack(
    target: &PackTarget,
    opts: &PackOptions,
    tx: Sender<PackEvent>,
    job_id: &str,
    cancel: CancellationToken,
) -> CoreResult<PackResult> {
    let start = Instant::now();
    let mut warnings: Vec<PackWarning> = Vec::new();

    // _clone_guard keeps the GitHub TempDir alive for the duration of pack();
    // dropping it at end-of-fn cleans up the cloned repo. None for Folder targets.
    let (root, label, _clone_guard) = resolve_target(target, job_id, &tx)?;

    let _ = tx.send(ProgressEvent::Started {
        job_id: job_id.into(),
        target_label: label.clone(),
    });

    let walk_start = Instant::now();
    let matcher = IgnoreMatcher::new(&root, &opts.custom_ignore_patterns, opts.respect_gitignore);
    let mut outcome = walker::walk(
        &root,
        &matcher,
        &WalkOptions {
            max_file_size_kb: opts.max_file_size_kb,
        },
    );

    // ── Pin pre-pass ─────────────────────────────────────────────────────────
    // Resolve all instructional files that exist under root and decide which
    // ones need to be force-included (i.e. weren't already picked up by the
    // walker). We also build the set of pinned paths so we can reorder entries
    // afterwards (pinned entries render first).
    let pinned_rel_paths: Vec<String> = pin::pinned_files(&root);

    // Build a fast lookup of paths already in outcome.included.
    let mut already_included: HashSet<String> = outcome
        .included
        .iter()
        .map(|f| f.path.clone())
        .collect();

    let mut pinned_set: HashSet<String> = HashSet::new();

    for pinned_path in &pinned_rel_paths {
        pinned_set.insert(pinned_path.clone());

        if already_included.contains(pinned_path) {
            // Already included by the walker — nothing to add, just track it.
            continue;
        }

        // Not included by the walker. Check whether the user tier explicitly
        // excludes it. If so, respect that and skip.
        let native_path = std::path::Path::new(pinned_path);
        if matcher.is_user_ignored(native_path, false) {
            // User explicitly excluded this pinned file — honour it.
            continue;
        }

        // Force-include: the file exists (pinned_files already checked) and
        // the user hasn't blocked it. Read metadata for bytes.
        let abs = root.join(pinned_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let bytes = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);

        // Remove from skipped if it appears there (e.g. was blocked by builtin/project tier).
        outcome.skipped.retain(|(p, _)| p != pinned_path);

        outcome.included.push(FileFound {
            path: pinned_path.clone(),
            bytes,
        });
        already_included.insert(pinned_path.clone());
    }
    // ── End pin pre-pass ──────────────────────────────────────────────────────
    let walk_ms = walk_start.elapsed().as_millis() as u32;

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

    // Checkpoint 1: after walk, before processing.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    let _ = tx.send(ProgressEvent::BuildingOutput);

    let process_start = Instant::now();
    let results: Vec<(FileEntry, Vec<PackWarning>)> = outcome
        .included
        .par_iter()
        .map(|f| {
            let abs = root.join(&f.path);
            let mut file_warnings: Vec<PackWarning> = Vec::new();

            // Per-file cancellation check.
            if cancel.is_cancelled() {
                return (
                    FileEntry {
                        path: f.path.clone(),
                        content: String::new(),
                        bytes: 0,
                        tokens: None,
                        hash: String::new(),
                    },
                    file_warnings,
                );
            }

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

            // Hash the original bytes BEFORE any comment-strip / compress transforms,
            // so we can move `raw` into `after_comments` below without an extra clone.
            //
            // Per-file `tokens` is computed AFTER the secret-scan loop so
            // it describes the same (post-redaction) content as
            // `tokens_total` and `tokens_per_model`. See the post-redaction
            // pass below.
            let hash = hash_content(raw.as_bytes(), &abs);

            // Step 1: strip comments if requested (tree-sitter languages only).
            let after_comments: String = if opts.remove_comments {
                if let Some(lang) = tree_sitter_compress::detect_language(&f.path) {
                    tree_sitter_compress::remove_comments(&raw, lang)
                } else {
                    raw
                }
            } else {
                raw
            };

            // Step 2: optionally compress to a skeleton.
            let content: String = if opts.compress {
                if let Some(lang) = tree_sitter_compress::detect_language(&f.path) {
                    tree_sitter_compress::compress(&after_comments, lang)
                } else {
                    after_comments
                }
            } else {
                after_comments
            };

            (
                FileEntry {
                    path: f.path.clone(),
                    content,
                    bytes: f.bytes,
                    tokens: None,
                    hash,
                },
                file_warnings,
            )
        })
        .collect();
    let process_ms = process_start.elapsed().as_millis() as u32;

    // Checkpoint 2: after process loop.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    let mut entries: Vec<FileEntry> = Vec::with_capacity(results.len());
    for (entry, w) in results {
        entries.push(entry);
        warnings.extend(w);
    }

    // ── Pin reorder ───────────────────────────────────────────────────────────
    // Partition entries into (pinned, non-pinned), then reassemble with pinned
    // entries in declaration order first, non-pinned in their original walk order.
    //
    // Index-permutation + `mem::take`: we never clone a FileEntry. Each entry
    // is moved exactly once into the `taken` staging Vec (leaving an empty
    // `Default::default()` placeholder in its original slot), then drained
    // back into `entries` in permutation order via a second `mem::take`.
    {
        // Build a lookup from path → index in the `entries` Vec.
        let mut path_to_idx: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            path_to_idx.insert(e.path.as_str(), i);
        }

        // Collect pinned indices in declaration order (skipping absent + dedup'd).
        let mut pinned_indices: Vec<usize> = Vec::new();
        let mut pinned_seen: HashSet<usize> = HashSet::new();
        for rel in &pinned_rel_paths {
            if let Some(&idx) = path_to_idx.get(rel.as_str()) {
                if pinned_seen.insert(idx) {
                    pinned_indices.push(idx);
                }
            }
        }

        // Non-pinned indices in their original walk order.
        let non_pinned_indices: Vec<usize> = (0..entries.len())
            .filter(|i| !pinned_seen.contains(i))
            .collect();

        // Final permutation: pinned-first, then non-pinned.
        let perm: Vec<usize> = pinned_indices
            .into_iter()
            .chain(non_pinned_indices)
            .collect();

        // `path_to_idx` borrows `entries` immutably; drop it before we mutate.
        drop(path_to_idx);

        // Move each entry exactly once via `mem::take` + perm-driven reassembly.
        // First pass: take every slot into `taken` (originals are now Default).
        // Second pass: take from `taken[perm[i]]` to build the final order.
        let mut taken: Vec<FileEntry> = entries.iter_mut().map(std::mem::take).collect();
        entries = perm
            .into_iter()
            .map(|i| std::mem::take(&mut taken[i]))
            .collect();
    }
    // ── End pin reorder ───────────────────────────────────────────────────────

    // pinned_count: how many entries at the front of `entries` are pinned files.
    // Passed to markdown/plain renderers so they can keep the pinned segment in
    // declaration order while sorting the non-pinned tail alphabetically.
    //
    // Relies on the invariant that the pin reorder block above produces a
    // contiguous pinned-then-non-pinned layout. The debug_assert below catches
    // any future refactor that breaks that contiguity.
    let pinned_count = entries
        .iter()
        .take_while(|e| pinned_set.contains(&e.path))
        .count();
    debug_assert_eq!(
        pinned_count,
        entries.iter().filter(|e| pinned_set.contains(&e.path)).count(),
        "pin reorder must produce a contiguous pinned prefix",
    );

    let mut secrets_found = 0u32;
    let mut all_redactions: Vec<PackRedaction> = Vec::new();
    let secret_scan_ms: Option<u32> = if opts.secret_scan {
        let secret_scan_start = Instant::now();
        // Hoisting `vendored()` out keeps the keyword-index cache hot across files.
        let ruleset = secrets::ruleset::vendored();

        // Parallel pass: each thread owns one entry slot via `par_iter_mut`,
        // mutates content in-place, and produces a `Vec<Redaction>` for that
        // entry. The whole `entries` slice is borrowed mutably only by the
        // iterator, which gives non-overlapping `&mut` to each slot.
        let per_file: Vec<Vec<crate::secrets::Redaction>> = entries
            .par_iter_mut()
            .map(|e| {
                let result = secrets::scan_and_redact(&e.content, ruleset);
                // Replace original content with redacted content so the pack
                // output ships the redacted version, not the secrets.
                e.content = result.redacted_content;
                result.redactions
            })
            .collect();

        // Serial post-pass: emit progress events in deterministic file-order,
        // increment the `secrets_found` counter, and append `all_redactions`
        // in stable order for the security_report block + PackResult. This
        // loop is O(total_redactions), not O(N_files), so it is cheap.
        for (e, redactions) in entries.iter().zip(per_file.iter()) {
            for r in redactions {
                secrets_found += 1;
                let _ = tx.send(ProgressEvent::SecretHit {
                    path: e.path.clone(),
                    secret_kind: r.rule_id.clone(),
                    line: r.line,
                });
                all_redactions.push(PackRedaction {
                    file: e.path.clone(),
                    rule_id: r.rule_id.clone(),
                    line: r.line,
                    byte_offset: r.byte_offset,
                });
            }
        }
        Some(secret_scan_start.elapsed().as_millis() as u32)
    } else {
        None
    };

    // Per-file token counts run AFTER the secret-scan loop so each entry's
    // `tokens` reflects the same (post-redaction) content as `tokens_total`
    // and `tokens_per_model`. Parallel pass — the encoder behind
    // `count_by_name` is `&'static` (cached via `OnceLock<CoreBPE>`) and
    // each thread mutates only its own entry slot.
    //
    // `tokens_per_model` is always `Some(_)` when `count_tokens` is on so
    // the UI can distinguish "count_tokens disabled" from "count_tokens
    // enabled but a tokenizer hiccupped" — the latter falls through to a
    // zero-filled struct (effectively unreachable in practice). Both
    // `tokens_per_model` and `tokens_total` reflect joined per-file content,
    // NOT the final emitted output: pack overhead (XML/MD wrappers, stats
    // block, protocol envelope) typically adds 5–15% on top.
    let (tokens_per_model, tokenize_ms) = if opts.count_tokens {
        let tokenize_start = Instant::now();
        // Parallel per-file tokenization: each thread owns one entry slot
        // via `par_iter_mut` and writes back its own `tokens`. Warnings are
        // collected as `Vec<Option<PackWarning>>` (one slot per entry to
        // preserve order) and flattened back into `warnings` after.
        let tokenize_warnings: Vec<Option<PackWarning>> = entries
            .par_iter_mut()
            .map(|e| {
                match tokens::count_by_name(&opts.tokenizer_model, &e.content) {
                    Ok(n) => {
                        e.tokens = Some(n);
                        None
                    }
                    Err(err) => {
                        e.tokens = None;
                        Some(PackWarning {
                            kind: WarningKind::TokenizeFailed,
                            path: Some(e.path.clone()),
                            message: format!("token count failed: {err}"),
                        })
                    }
                }
            })
            .collect();
        warnings.extend(tokenize_warnings.into_iter().flatten());
        let joined: String = entries
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let per_model = Some(tokens::count_all(&joined).unwrap_or_default());
        (per_model, Some(tokenize_start.elapsed().as_millis() as u32))
    } else {
        (None, None)
    };

    // Use u64 accumulator — `tokens_total` is u32 on the wire, but a multi-
    // million-token monorepo pack can wrap a u32 sum mid-loop. Saturate at
    // the cast site instead of silently wrapping.
    let mut bytes_total = 0u64;
    let mut tokens_total: u64 = 0;
    for e in &entries {
        bytes_total += e.bytes;
        if let Some(t) = e.tokens {
            tokens_total += u64::from(t);
        }
    }
    let tokens_total: u32 = tokens_total.min(u64::from(u32::MAX)) as u32;

    // files_total accounting:
    //   included = files we kept (walker matches + force-included pins)
    //   skipped  = files we explicitly excluded (after pin pre-pass removed
    //              any pinned-but-skipped entries from this list)
    //   total    = included + skipped
    //
    // The pin pre-pass mutates outcome.included (push) and outcome.skipped
    // (retain), keeping the invariant `total = included + skipped` stable
    // across pinning. A pinned file that wasn't visited by the walker is a
    // net-add to total (it's a new file we wouldn't have counted otherwise),
    // which is correct.
    // First construction: real per-phase fields are known, but `emit_ms` cannot
    // be measured until after the renderer runs below. We use `emit_ms: 0` and
    // then refresh `stats` with the real `emit_ms` (and an updated total
    // `duration_ms`) immediately after the emit match. Both `duration_ms` and
    // `emit_ms` therefore reflect the post-emit wall clock in the final stats
    // shipped to the renderer's `<security_report>`/UI; the renderer only sees
    // the pre-emit version, but its `emit_ms` is never serialized into the
    // pack output (renderers don't read that field today).
    let stats = PackStats {
        files_total: (outcome.included.len() + outcome.skipped.len()) as u32,
        files_included: entries.len() as u32,
        files_skipped: outcome.skipped.len() as u32,
        bytes_total,
        tokens_total: opts.count_tokens.then_some(tokens_total),
        tokens_per_model,
        secrets_found,
        duration_ms: start.elapsed().as_millis() as u32,
        walk_ms,
        process_ms,
        secret_scan_ms,
        tokenize_ms,
        emit_ms: 0,
    };

    let dir_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();

    let emit_start = Instant::now();
    let output = match opts.format {
        PackFormat::Xml => {
            let protocol_block = protocol::block_for_pack(&opts.goal, &opts.protocol_version)?;
            let mut builder = XmlBuilder::with_capacity((stats.bytes_total as usize).saturating_mul(2));
            builder
                .open_repository()
                .raw_block(&protocol_block)
                .stats_block(&label, opts, &stats, &entries, &all_redactions)
                .security_report_block(&all_redactions)
                .directory_structure(&dir_paths);
            // Route to the Anthropic cxml schema (default) or the legacy schema.
            match opts.xml_schema {
                XmlSchema::Cxml => { builder.documents(&entries); }
                XmlSchema::Legacy => { builder.files_legacy(&entries); }
            }
            builder.close_repository();
            builder.finish()
        }
        PackFormat::Markdown => {
            markdown::render(&label, opts, &stats, &entries, pinned_count, &all_redactions)
        }
        PackFormat::PlainText => {
            plain::render(&label, opts, &stats, &entries, pinned_count, &all_redactions)
        }
    };
    let emit_ms = emit_start.elapsed().as_millis() as u32;

    // Refresh stats with real emit_ms and updated total duration_ms.
    let stats = PackStats {
        emit_ms,
        duration_ms: start.elapsed().as_millis() as u32,
        ..stats
    };

    let claude_code_prompt = protocol::claude_code_prompt(&opts.protocol_version)?;

    // Checkpoint 3: after emit, before returning result.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    let _ = tx.send(ProgressEvent::Done {
        stats: stats.clone(),
    });

    Ok(PackResult {
        output,
        claude_code_prompt,
        stats,
        warnings,
        redactions: all_redactions,
    })
}

/// Hash file content using BLAKE3.
///
/// For files ≤256 KB the bytes are already in memory (from `read_text_with_fallback`),
/// so we hash in-process. For larger files we use `update_mmap_rayon` to parallelise
/// the read; the mmap path takes the on-disk file so we pass the path through.
const MMAP_THRESHOLD: usize = 256 * 1024; // 256 KB

fn hash_content(bytes: &[u8], path: &Path) -> String {
    if bytes.len() <= MMAP_THRESHOLD {
        let digest = blake3::hash(bytes);
        digest.to_hex().to_string()
    } else {
        let mut hasher = blake3::Hasher::new();
        // update_mmap_rayon returns a Result; fall back to in-memory on error.
        if hasher.update_mmap_rayon(path).is_err() {
            hasher.update(bytes);
        }
        hasher.finalize().to_hex().to_string()
    }
}

fn resolve_target(
    target: &PackTarget,
    job_id: &str,
    tx: &Sender<PackEvent>,
) -> CoreResult<(std::path::PathBuf, String, Option<crate::github::ClonedRepo>)> {
    match target {
        PackTarget::Folder(p) => Ok((p.clone(), p.display().to_string(), None)),
        PackTarget::GitHub(url) => {
            let _ = tx.send(ProgressEvent::Cloning { progress_pct: 0 });
            let parsed = crate::github::parse_github_url(url)?;
            let label = format!("github.com/{}/{}", parsed.owner, parsed.repo);
            let cloned = crate::github::shallow_clone(url, job_id)?;
            Ok((cloned.path.clone(), label, Some(cloned)))
        }
    }
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

    // BLAKE3 known test vector: empty input → this exact digest.
    // Source: https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json
    #[test]
    fn blake3_empty_input_matches_known_vector() {
        let digest = hash_content(b"", std::path::Path::new(""));
        assert_eq!(
            digest,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

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
        let result = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-test", CancellationToken::new()).unwrap();
        assert!(result.output.contains("<protocol version=\"grok-to-cc-v1\">"));
        assert!(result.output.contains("<documents>"));
        assert!(result.output.contains("<document "));
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
        let _ = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-test", CancellationToken::new()).unwrap();
        let mut events: Vec<&'static str> = Vec::new();
        for ev in rx.try_iter() {
            events.push(match ev {
                ProgressEvent::Started { .. } => "started",
                ProgressEvent::Walking { .. } => "walking",
                ProgressEvent::FileFoundBatch { .. } => "batch",
                ProgressEvent::FileSkipped { .. } => "skipped",
                ProgressEvent::BuildingOutput => "building",
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
        let result = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-test", CancellationToken::new()).unwrap();

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

    #[test]
    fn pack_populates_per_phase_timing_fields_when_all_enabled() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            secret_scan: true,
            count_tokens: true,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts, tx, "job-test", CancellationToken::new(),
        ).unwrap();
        assert!(result.stats.secret_scan_ms.is_some(), "secret_scan_ms must be Some when enabled, got None");
        assert!(result.stats.tokenize_ms.is_some(), "tokenize_ms must be Some when enabled, got None");

        // Real timing sanity check: phases run sequentially inside pack(), so
        // their elapsed-times must sum to no more than the total wall clock
        // (plus a small slack for stats construction + the brief gap between
        // phases). A bug that wired all timers to 0 would fail this check
        // because duration_ms is independently measured.
        let phase_sum: u32 = result.stats.walk_ms
            + result.stats.process_ms
            + result.stats.secret_scan_ms.unwrap_or(0)
            + result.stats.tokenize_ms.unwrap_or(0)
            + result.stats.emit_ms;
        assert!(
            phase_sum <= result.stats.duration_ms + 100,
            "phase_sum ({}ms = walk {} + process {} + scan {:?} + tokenize {:?} + emit {}) \
             must be <= duration_ms ({}ms) + 100ms slack",
            phase_sum,
            result.stats.walk_ms,
            result.stats.process_ms,
            result.stats.secret_scan_ms,
            result.stats.tokenize_ms,
            result.stats.emit_ms,
            result.stats.duration_ms,
        );
        // Each phase elapsed must individually fit within the total. A bug
        // that left a stale "huge" timer value from a previous run would
        // trip this.
        assert!(result.stats.walk_ms <= result.stats.duration_ms);
        assert!(result.stats.process_ms <= result.stats.duration_ms);
        assert!(result.stats.emit_ms <= result.stats.duration_ms);
    }

    #[test]
    fn pack_omits_secret_scan_ms_when_secret_scan_disabled() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            secret_scan: false,
            count_tokens: true,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts, tx, "job-test", CancellationToken::new(),
        ).unwrap();
        assert!(result.stats.secret_scan_ms.is_none(), "secret_scan_ms must be None when disabled, got {:?}", result.stats.secret_scan_ms);
        assert!(result.stats.tokenize_ms.is_some());
    }

    #[test]
    fn pack_omits_tokenize_ms_when_count_tokens_disabled() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            secret_scan: true,
            count_tokens: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts, tx, "job-test", CancellationToken::new(),
        ).unwrap();
        assert!(result.stats.tokenize_ms.is_none(), "tokenize_ms must be None when disabled, got {:?}", result.stats.tokenize_ms);
        assert!(result.stats.secret_scan_ms.is_some());
    }

    /// Verify that a pre-cancelled token causes pack() to return Err(Cancelled)
    /// immediately, without processing any files.
    #[test]
    fn pack_returns_cancelled_when_token_is_pre_cancelled() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            secret_scan: false,
            count_tokens: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let cancel = CancellationToken::new();
        cancel.cancel(); // signal before pack() starts

        let start = std::time::Instant::now();
        let result = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-cancel-test", cancel);
        let elapsed = start.elapsed();

        assert!(
            matches!(result, Err(CoreError::Cancelled)),
            "expected Err(Cancelled), got {:?}",
            result
        );
        assert!(
            elapsed.as_millis() < 500,
            "pack() took too long after cancellation: {:?}",
            elapsed
        );
    }

    // ── Task E tests ──────────────────────────────────────────────────────────

    /// Pinned file (`AGENTS.md`) must appear before a non-pinned file (`random.md`)
    /// in the pack entries (proves reordering).
    #[test]
    fn pack_pinned_file_rendered_before_normal_files() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("AGENTS.md"), "# Agent instructions\n").unwrap();
        fs::write(d.path().join("random.md"), "# Just a regular file\n").unwrap();

        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: false,
            secret_scan: false,
            respect_gitignore: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts,
            tx,
            "job-pin-order",
            CancellationToken::new(),
        )
        .unwrap();

        // The output must contain AGENTS.md before random.md.
        let agents_pos = result.output.find("AGENTS.md").expect("AGENTS.md not in output");
        let random_pos = result.output.find("random.md").expect("random.md not in output");
        assert!(
            agents_pos < random_pos,
            "AGENTS.md must appear before random.md in the output"
        );
        assert_eq!(result.stats.files_included, 2);
    }

    /// `tokens_per_model` must be `Some(_)` when count_tokens=true and the
    /// per-model counts must include non-zero numbers for all 7 model rows.
    #[test]
    fn pack_emits_tokens_per_model_when_count_tokens_enabled() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: true,
            secret_scan: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts,
            tx,
            "job-tokens-per-model",
            CancellationToken::new(),
        )
        .unwrap();

        let tpm = result
            .stats
            .tokens_per_model
            .expect("tokens_per_model should be Some when count_tokens is true");
        // Smoke-check that all 7 fields received non-zero counts on the
        // joined fixture content (a.rs + README.md).
        assert!(tpm.gpt4o > 0);
        assert!(tpm.claude > 0);
        assert!(tpm.llama3 > 0);
        assert!(tpm.qwen2_5 > 0);
        assert!(tpm.deep_seek > 0);
        assert!(tpm.mistral > 0);
        assert!(tpm.gemini_approx > 0);
        // GeminiApprox is cl100k × 1.05 ceil — always >= gpt4o.
        assert!(tpm.gemini_approx >= tpm.gpt4o);
        // gpt4o and claude share the cl100k encoder.
        assert_eq!(tpm.gpt4o, tpm.claude);
    }

    /// `tokens_per_model` must remain `Some(_)` when count_tokens=true even
    /// if every file is filtered out — `None` is reserved for "count_tokens
    /// is off". A zero-filled struct is the correct shape for the empty case.
    #[test]
    fn pack_emits_tokens_per_model_even_when_all_files_excluded() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("only.rs"), "fn main() {}\n").unwrap();

        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: true,
            secret_scan: false,
            respect_gitignore: false,
            custom_ignore_patterns: vec!["only.rs".into()],
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts,
            tx,
            "job-tokens-per-model-empty",
            CancellationToken::new(),
        )
        .unwrap();

        assert_eq!(result.stats.files_included, 0);
        let tpm = result
            .stats
            .tokens_per_model
            .expect("tokens_per_model must be Some when count_tokens=true, even with zero entries");
        // Empty joined content → all-zero counts.
        assert_eq!(tpm.gpt4o, 0);
        assert_eq!(tpm.claude, 0);
        assert_eq!(tpm.llama3, 0);
        assert_eq!(tpm.qwen2_5, 0);
        assert_eq!(tpm.deep_seek, 0);
        assert_eq!(tpm.mistral, 0);
        assert_eq!(tpm.gemini_approx, 0);
    }

    /// `tokens_per_model` must be `None` when count_tokens=false.
    #[test]
    fn pack_omits_tokens_per_model_when_count_tokens_disabled() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: false,
            secret_scan: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts,
            tx,
            "job-tokens-per-model-off",
            CancellationToken::new(),
        )
        .unwrap();

        assert!(
            result.stats.tokens_per_model.is_none(),
            "tokens_per_model should be None when count_tokens is false"
        );
        assert!(result.stats.tokens_total.is_none());
    }

    /// User explicitly excluding a pinned file via `custom_ignore_patterns` must
    /// be respected — the file must NOT appear in the pack output.
    #[test]
    fn pack_respects_user_excluding_a_pinned_file() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("AGENTS.md"), "# Agent instructions\n").unwrap();

        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: false,
            secret_scan: false,
            respect_gitignore: false,
            custom_ignore_patterns: vec!["AGENTS.md".into()],
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts,
            tx,
            "job-pin-excluded",
            CancellationToken::new(),
        )
        .unwrap();

        // AGENTS.md must NOT appear in the output.
        // (The <files> block should not contain it; file path search is fine.)
        assert!(
            !result.output.contains("<file path=\"AGENTS.md\""),
            "AGENTS.md must be absent when user-excluded via custom_ignore_patterns"
        );
        assert_eq!(
            result.stats.files_included, 0,
            "no files should be included when the only file is user-excluded"
        );
    }
}
