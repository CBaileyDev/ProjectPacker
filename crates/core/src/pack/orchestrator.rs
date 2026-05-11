use crate::error::{CoreError, CoreResult};
use crate::ignore::IgnoreMatcher;
use crate::pack::pin;
use crate::pack::xml::XmlBuilder;
use crate::pack::{markdown, plain};
use crate::pack::FileEntry;
use crate::protocol;
use crate::secrets;
use crate::tokens;
use crate::tokens::TokensPerModel;
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
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub type PackEvent = ProgressEvent;

/// Coalesce + batch + group window for the IPC throttler.
///
/// Walking events coalesce to one emission per [`THROTTLE_WINDOW`]; SecretHit
/// items are emitted in groups bounded by the same window. FileFound items
/// flush as a single [`ProgressEvent::FileFoundBatch`] at [`FILE_FOUND_BATCH`]
/// (or on `flush_all` / drop).
const THROTTLE_WINDOW: Duration = Duration::from_millis(100);
const FILE_FOUND_BATCH: usize = 50;

/// Internal classification used while batching. Variants that need throttling
/// carry the minimal data needed to reconstruct the public `ProgressEvent`;
/// `Other` is a pass-through bucket for events that ship unchanged.
///
/// This enum is private — the wire format is `ProgressEvent` and is not
/// affected.
#[allow(dead_code)]
enum ProgressEventDelta {
    Walking(u32),
    FileFound(FileFound),
    SecretHit { path: String, kind: String, line: u32 },
    Other(ProgressEvent),
}

/// IPC event throttler. Cuts UI flooding from three sources:
///
/// 1. **Walking events** — coalesced to one emission per [`THROTTLE_WINDOW`].
///    Drop intermediate `files_scanned` values; on flush emit the most-recent.
/// 2. **FileFound items** — buffered up to [`FILE_FOUND_BATCH`], then flushed
///    as a single [`ProgressEvent::FileFoundBatch`]. Force-flushed on phase
///    transitions and on `Drop`.
/// 3. **SecretHit items** — buffered and flushed as a group at most every
///    [`THROTTLE_WINDOW`]. Each flush sends individual `SecretHit` events
///    back-to-back (preserves wire format) but the group rate is throttled.
///
/// Pass-through events (Started, BuildingOutput, Done, Error, etc.) bypass
/// the throttler entirely via [`Self::send_passthrough`].
struct EventThrottler {
    tx: Sender<ProgressEvent>,
    // Walking coalescing.
    pending_walking: Option<u32>,
    last_walking_emit: Option<Instant>,
    // FileFound batching.
    file_found_buf: Vec<FileFound>,
    // SecretHit grouping.
    secret_hit_buf: Vec<(String, String, u32)>,
    last_secret_emit: Option<Instant>,
}

impl EventThrottler {
    fn new(tx: Sender<ProgressEvent>) -> Self {
        Self {
            tx,
            pending_walking: None,
            last_walking_emit: None,
            file_found_buf: Vec::with_capacity(FILE_FOUND_BATCH),
            secret_hit_buf: Vec::new(),
            last_secret_emit: None,
        }
    }

    /// Borrow the underlying tx for low-frequency pass-through events
    /// (transform lifecycle, etc.). The throttler's own buffers are NOT
    /// flushed first — callers must ensure that's correct for their event.
    /// For TransformStart/Done this is safe: those events have no ordering
    /// constraint with FileFound or SecretHit.
    fn passthrough_tx(&self) -> &Sender<ProgressEvent> {
        &self.tx
    }

    /// Pass an event straight through to the underlying channel without
    /// throttling. Used for Started/Cloning/BuildingOutput/Done/Error/etc.
    /// Flushes any pending throttled state first so the wire-order invariants
    /// hold (e.g. a buffered FileFoundBatch must arrive before BuildingOutput).
    fn send_passthrough(&mut self, ev: ProgressEvent) {
        self.flush_all();
        let _ = self.tx.send(ev);
    }

    /// Coalesce a Walking update. Emits at most one Walking event per
    /// [`THROTTLE_WINDOW`]; intermediate values are dropped. The most-recent
    /// `files_scanned` wins.
    fn push_walking(&mut self, scanned: u32) {
        self.pending_walking = Some(scanned);
        let now = Instant::now();
        let due = self
            .last_walking_emit
            .is_none_or(|t| now.duration_since(t) >= THROTTLE_WINDOW);
        if due {
            self.flush_walking_now();
        }
    }

    fn flush_walking_now(&mut self) {
        if let Some(scanned) = self.pending_walking.take() {
            let _ = self.tx.send(ProgressEvent::Walking { files_scanned: scanned });
            self.last_walking_emit = Some(Instant::now());
        }
    }

    /// Buffer one FileFound item; flush as a `FileFoundBatch` once the buffer
    /// hits [`FILE_FOUND_BATCH`].
    fn push_file_found(&mut self, item: FileFound) {
        self.file_found_buf.push(item);
        if self.file_found_buf.len() >= FILE_FOUND_BATCH {
            self.flush_file_found();
        }
    }

    fn flush_file_found(&mut self) {
        if !self.file_found_buf.is_empty() {
            let batch = std::mem::take(&mut self.file_found_buf);
            let _ = self.tx.send(ProgressEvent::FileFoundBatch { paths: batch });
        }
    }

    /// Buffer one SecretHit. Flushes the buffered group once at most every
    /// [`THROTTLE_WINDOW`]; each flush emits individual `SecretHit` events
    /// back-to-back (preserves wire format).
    fn push_secret_hit(&mut self, path: String, kind: String, line: u32) {
        self.secret_hit_buf.push((path, kind, line));
        let now = Instant::now();
        let due = self
            .last_secret_emit
            .is_none_or(|t| now.duration_since(t) >= THROTTLE_WINDOW);
        if due {
            self.flush_secret_hits();
        }
    }

    fn flush_secret_hits(&mut self) {
        if !self.secret_hit_buf.is_empty() {
            for (path, kind, line) in self.secret_hit_buf.drain(..) {
                let _ = self.tx.send(ProgressEvent::SecretHit {
                    path,
                    secret_kind: kind,
                    line,
                });
            }
            self.last_secret_emit = Some(Instant::now());
        }
    }

    /// Force-flush all buffered state in deterministic order:
    /// Walking → FileFoundBatch → SecretHit. Called on phase transitions and
    /// from `Drop`.
    fn flush_all(&mut self) {
        self.flush_walking_now();
        self.flush_file_found();
        self.flush_secret_hits();
    }
}

impl Drop for EventThrottler {
    fn drop(&mut self) {
        self.flush_all();
    }
}

pub fn pack(
    target: &PackTarget,
    opts: &PackOptions,
    tx: Sender<PackEvent>,
    job_id: &str,
    cancel: CancellationToken,
    // PAT for cloning private GitHub repos. Read from the OS keychain
    // by the app layer; never crosses the JS↔Rust boundary or appears in
    // serialized state. None for folder targets / public repos.
    github_token: Option<&str>,
) -> CoreResult<PackResult> {
    let start = Instant::now();
    let mut warnings: Vec<PackWarning> = Vec::new();

    // _clone_guard keeps the GitHub TempDir alive for the duration of pack();
    // dropping it at end-of-fn cleans up the cloned repo. None for Folder targets.
    // resolve_target may emit `Cloning` for GitHub targets BEFORE `Started` —
    // preserving the historical event order.
    //
    // resolve_target needs the raw `tx` clone (the throttler isn't built yet at
    // that point and `Cloning` is a pass-through event anyway).
    let (root, label, _clone_guard) = resolve_target(target, job_id, &tx, github_token)?;

    // All subsequent IPC events route through the throttler. The throttler
    // owns a clone of `tx` and holds its own buffers; on drop (end of pack()
    // or early-return) it flushes everything.
    let mut throttler = EventThrottler::new(tx);

    throttler.send_passthrough(ProgressEvent::Started {
        job_id: job_id.into(),
        target_label: label.clone(),
    });

    let (outcome, pinned_rel_paths, pinned_set, walk_ms) = run_walk_phase(&root, opts);

    // Coalesce + batch the walk-phase events. `push_walking` may drop intermediate
    // values; `push_file_found` flushes at every FILE_FOUND_BATCH boundary and the
    // remainder ships when `BuildingOutput` triggers a flush below.
    throttler.push_walking(outcome.included.len() as u32);
    for f in &outcome.included {
        throttler.push_file_found(f.clone());
    }
    // Per-file FileSkipped events used to be emitted here in a tight loop. On
    // repos with build artifacts (target/, node_modules, .git/objects), the
    // walker's skipped list can be tens of thousands of entries — each event
    // crosses the IPC bridge and triggers a Zustand+React update, freezing
    // the UI for minutes. The renderer drops these events anyway (see
    // ProgressLog.tsx) and the final stats.files_skipped count is preserved
    // in PackResult.

    // Checkpoint 1: after walk, before processing.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    // BuildingOutput is a phase boundary — flush everything queued first.
    throttler.send_passthrough(ProgressEvent::BuildingOutput);

    let (mut entries, process_warnings, process_ms) =
        run_process_phase(&outcome, opts, &root, &cancel);
    warnings.extend(process_warnings);

    // Checkpoint 2: after process loop.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    let pinned_count = apply_pin_reorder(&mut entries, &pinned_rel_paths, &pinned_set);

    // Checkpoint: between pin-reorder and secret-scan.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    let (transform_reports, transform_phase_ms) =
        crate::transforms::run_transform_phase(&mut entries, opts, throttler.passthrough_tx());

    let (secrets_found, all_redactions, secret_scan_ms) =
        run_secret_scan_phase(&mut entries, opts, &mut throttler);

    let (tokens_per_model, tokenize_ms, tokenize_warnings) =
        run_tokenize_phase(&mut entries, opts);
    warnings.extend(tokenize_warnings);

    let (bytes_total, tokens_total) = accumulate_byte_token_totals(&entries);

    // files_total accounting:
    //   included = files we kept (walker matches + force-included pins)
    //   skipped  = files we explicitly excluded (after pin pre-pass removed
    //              any pinned-but-skipped entries from this list)
    //   total    = included + skipped
    //
    // First construction: real per-phase fields are known, but `emit_ms` cannot
    // be measured until after the renderer runs below. We use `emit_ms: 0` and
    // then refresh `stats` with the real `emit_ms` (and an updated total
    // `duration_ms`) immediately after the emit phase. Both `duration_ms` and
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
        transforms: transform_reports.clone(),
        transform_phase_ms,
    };

    let (output, emit_ms) =
        run_emit_phase(&entries, &stats, opts, &label, &all_redactions, pinned_count)?;

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

    // Flush any remaining throttler buffers (trailing SecretHits, a
    // partial FileFoundBatch, the last Walking value) so they reach the
    // renderer before pack() returns. The throttler's own Drop also
    // flushes, but doing it here keeps wire-order deterministic.
    //
    // Done is intentionally NOT emitted here. The app layer (commands::
    // pack_start) sends Done AFTER `store_result` has stashed the
    // PackResult in the registry — otherwise the renderer can react to
    // Done and call `pack_get_result` faster than the spawn_blocking
    // task can reach the next line, producing a spurious "no result for
    // job …" error.
    throttler.flush_all();
    drop(throttler);

    Ok(PackResult {
        output,
        claude_code_prompt,
        stats,
        warnings,
        redactions: all_redactions,
    })
}

/// Walker + pin pre-pass. Target resolution + the `Started` event happen
/// in `pack()` directly so the historical event order (`Cloning` →
/// `Started` → walker) is preserved without threading the channel down here.
///
/// Returns `(outcome, pinned_rel_paths, pinned_set, walk_ms)`.
fn run_walk_phase(
    root: &Path,
    opts: &PackOptions,
) -> (walker::WalkOutcome, Vec<String>, HashSet<String>, u32) {
    let walk_start = Instant::now();
    let matcher = IgnoreMatcher::new(root, &opts.custom_ignore_patterns, opts.respect_gitignore);
    let mut outcome = walker::walk(
        root,
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
    let pinned_rel_paths: Vec<String> = pin::pinned_files(root);

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

    (outcome, pinned_rel_paths, pinned_set, walk_ms)
}

/// Per-file processing in `par_iter`: read + encoding-fallback + comment-strip +
/// compress + hash. Returns `(entries, accumulated_warnings, process_ms)`.
fn run_process_phase(
    outcome: &walker::WalkOutcome,
    opts: &PackOptions,
    root: &Path,
    cancel: &CancellationToken,
) -> (Vec<FileEntry>, Vec<PackWarning>, u32) {
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

    let mut entries: Vec<FileEntry> = Vec::with_capacity(results.len());
    let mut warnings: Vec<PackWarning> = Vec::new();
    for (entry, w) in results {
        entries.push(entry);
        warnings.extend(w);
    }

    (entries, warnings, process_ms)
}

/// Reorder `entries` so pinned files appear first (in declaration order), then
/// non-pinned files in their original walk order. Returns the pinned-prefix
/// length: how many entries at the front of `entries` are pinned.
///
/// Index-permutation + `mem::take`: we never clone a `FileEntry`. Each entry
/// is moved exactly once into the `taken` staging Vec (leaving an empty
/// `Default::default()` placeholder in its original slot), then drained back
/// into `entries` in permutation order via a second `mem::take`. The pinned-
/// count returned is passed to markdown/plain renderers so they keep the
/// pinned segment in declaration order while alphabetically sorting the
/// non-pinned tail.
fn apply_pin_reorder(
    entries: &mut Vec<FileEntry>,
    pinned_rel_paths: &[String],
    pinned_set: &HashSet<String>,
) -> usize {
    // Build a lookup from path → index in the `entries` Vec.
    let mut path_to_idx: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        path_to_idx.insert(e.path.as_str(), i);
    }

    // Collect pinned indices in declaration order (skipping absent + dedup'd).
    let mut pinned_indices: Vec<usize> = Vec::new();
    let mut pinned_seen: HashSet<usize> = HashSet::new();
    for rel in pinned_rel_paths {
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
    *entries = perm
        .into_iter()
        .map(|i| std::mem::take(&mut taken[i]))
        .collect();

    // pinned_count: how many entries at the front of `entries` are pinned files.
    // Passed to markdown/plain renderers so they can keep the pinned segment in
    // declaration order while sorting the non-pinned tail alphabetically.
    //
    // Relies on the invariant that the reorder above produces a contiguous
    // pinned-then-non-pinned layout. The debug_assert below catches any future
    // refactor that breaks that contiguity.
    let pinned_count = entries
        .iter()
        .take_while(|e| pinned_set.contains(&e.path))
        .count();
    debug_assert_eq!(
        pinned_count,
        entries.iter().filter(|e| pinned_set.contains(&e.path)).count(),
        "pin reorder must produce a contiguous pinned prefix",
    );
    pinned_count
}

/// Parallel `scan_and_redact` + serial post-pass for events and aggregation.
///
/// Returns `(secrets_found, all_redactions, secret_scan_ms_or_None)`.
/// Returns `None` for `secret_scan_ms` when `opts.secret_scan` is false.
///
/// SecretHit events route through the throttler so high-secret-count repos
/// don't flood the IPC bridge with thousands of individual hits in a tight
/// loop — see [`EventThrottler::push_secret_hit`].
fn run_secret_scan_phase(
    entries: &mut [FileEntry],
    opts: &PackOptions,
    throttler: &mut EventThrottler,
) -> (u32, Vec<PackRedaction>, Option<u32>) {
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
                throttler.push_secret_hit(e.path.clone(), r.rule_id.clone(), r.line);
                all_redactions.push(PackRedaction {
                    file: e.path.clone(),
                    rule_id: r.rule_id.clone(),
                    line: r.line,
                    byte_offset: r.byte_offset,
                });
            }
        }
        // Phase boundary: flush any remaining buffered SecretHits so the
        // wire ordering aligns with the file-order post-pass above.
        throttler.flush_secret_hits();
        Some(secret_scan_start.elapsed().as_millis() as u32)
    } else {
        None
    };

    (secrets_found, all_redactions, secret_scan_ms)
}

/// Per-file `count_by_name` (parallel) + per-file `count_all` sum (parallel).
///
/// Returns `(tokens_per_model_or_None, tokenize_ms_or_None, tokenize_warnings)`.
/// Both `Option`s are `None` when `opts.count_tokens` is false.
///
/// Per-file token counts run AFTER the secret-scan loop so each entry's
/// `tokens` reflects the same (post-redaction) content as `tokens_total`
/// and `tokens_per_model`. The encoder behind `count_by_name` is `&'static`
/// (cached via `OnceLock<CoreBPE>`) and each thread mutates only its own
/// entry slot.
///
/// `tokens_per_model` is always `Some(_)` when `count_tokens` is on so
/// the UI can distinguish "count_tokens disabled" from "count_tokens
/// enabled but a tokenizer hiccupped" — the latter falls through to a
/// zero-filled struct (effectively unreachable in practice).
///
/// Per-model token counts are summed per-file rather than encoding a joined
/// string. This trades a typically-<1% loss in accuracy (inter-file token-merge
/// effects at file boundaries) for ~content-size reduction in peak memory and
/// for-free parallelization across cores. Documented in CHANGELOG as a behavior
/// note for users who pinned exact pre-v0.5 token numbers. Pack overhead
/// (XML/MD wrappers, stats block, protocol envelope) still adds 5–15% on top
/// of the reported numbers vs. the final emitted output.
fn run_tokenize_phase(
    entries: &mut [FileEntry],
    opts: &PackOptions,
) -> (Option<TokensPerModel>, Option<u32>, Vec<PackWarning>) {
    if !opts.count_tokens {
        return (None, None, Vec::new());
    }

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
    let warnings: Vec<PackWarning> = tokenize_warnings.into_iter().flatten().collect();

    // Parallel per-file count_all. Each file is encoded once across all 7 model
    // tokenizers; the per-file results are summed via saturating arithmetic into
    // a single TokensPerModel. This avoids the ~content-bytes peak-memory of
    // the previous joined-string approach and parallelizes across cores.
    let per_file_counts: Vec<TokensPerModel> = entries
        .par_iter()
        .map(|e| tokens::count_all(&e.content).unwrap_or_default())
        .collect();
    let mut acc = TokensPerModel::default();
    for c in per_file_counts {
        acc.gpt4o = acc.gpt4o.saturating_add(c.gpt4o);
        acc.claude = acc.claude.saturating_add(c.claude);
        acc.llama3 = acc.llama3.saturating_add(c.llama3);
        acc.qwen2_5 = acc.qwen2_5.saturating_add(c.qwen2_5);
        acc.deep_seek = acc.deep_seek.saturating_add(c.deep_seek);
        acc.mistral = acc.mistral.saturating_add(c.mistral);
        acc.gemini_approx = acc.gemini_approx.saturating_add(c.gemini_approx);
    }
    (Some(acc), Some(tokenize_start.elapsed().as_millis() as u32), warnings)
}

/// Accumulate `bytes_total` + `tokens_total` across `entries`.
///
/// Uses a u64 accumulator — `tokens_total` is u32 on the wire, but a multi-
/// million-token monorepo pack can wrap a u32 sum mid-loop. Saturate at
/// the cast site instead of silently wrapping.
fn accumulate_byte_token_totals(entries: &[FileEntry]) -> (u64, u32) {
    let mut bytes_total = 0u64;
    let mut tokens_total: u64 = 0;
    for e in entries {
        bytes_total += e.bytes;
        if let Some(t) = e.tokens {
            tokens_total += u64::from(t);
        }
    }
    let tokens_total: u32 = tokens_total.min(u64::from(u32::MAX)) as u32;
    (bytes_total, tokens_total)
}

/// Build the output string by routing to the appropriate emitter.
/// Returns `(output, emit_ms)`.
///
/// `stats` is the pre-emit version (with `emit_ms: 0` placeholder); the
/// caller refreshes it with the real `emit_ms` after this returns. The
/// renderer only sees the pre-emit version, but its `emit_ms` is never
/// serialized into the pack output (renderers don't read that field today).
fn run_emit_phase(
    entries: &[FileEntry],
    stats: &PackStats,
    opts: &PackOptions,
    label: &str,
    all_redactions: &[PackRedaction],
    pinned_count: usize,
) -> CoreResult<(String, u32)> {
    let dir_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();

    let emit_start = Instant::now();
    let output = match opts.format {
        PackFormat::Xml => {
            let protocol_block = protocol::block_for_pack(&opts.goal, &opts.protocol_version)?;
            let mut builder = XmlBuilder::with_capacity((stats.bytes_total as usize).saturating_mul(2));
            builder
                .open_repository()
                .raw_block(&protocol_block)
                .stats_block(label, opts, stats, entries, all_redactions)
                .security_report_block(all_redactions)
                .directory_structure(&dir_paths);
            // Route to the Anthropic cxml schema (default) or the legacy schema.
            match opts.xml_schema {
                XmlSchema::Cxml => { builder.documents(entries); }
                XmlSchema::Legacy => { builder.files_legacy(entries); }
            }
            builder.close_repository();
            builder.finish()
        }
        PackFormat::Markdown => {
            markdown::render(label, opts, stats, entries, pinned_count, all_redactions)
        }
        PackFormat::PlainText => {
            plain::render(label, opts, stats, entries, pinned_count, all_redactions)
        }
    };
    let emit_ms = emit_start.elapsed().as_millis() as u32;
    Ok((output, emit_ms))
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
    github_token: Option<&str>,
) -> CoreResult<(std::path::PathBuf, String, Option<crate::github::ClonedRepo>)> {
    match target {
        PackTarget::Folder(p) => Ok((p.clone(), p.display().to_string(), None)),
        PackTarget::GitHub(url) => {
            let _ = tx.send(ProgressEvent::Cloning { progress_pct: 0 });
            let parsed = crate::github::parse_github_url(url)?;
            let label = format!("github.com/{}/{}", parsed.owner, parsed.repo);
            let cloned = crate::github::shallow_clone_with_auth(url, job_id, github_token)?;
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
        let result = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-test", CancellationToken::new(), None).unwrap();
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
        let _ = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-test", CancellationToken::new(), None).unwrap();
        let mut events: Vec<&'static str> = Vec::new();
        for ev in rx.try_iter() {
            events.push(match ev {
                ProgressEvent::Started { .. } => "started",
                ProgressEvent::Walking { .. } => "walking",
                ProgressEvent::FileFoundBatch { .. } => "batch",
                ProgressEvent::FileSkipped { .. } => "skipped",
                ProgressEvent::BuildingOutput => "building",
                ProgressEvent::TransformStart { .. } => "transform_start",
                ProgressEvent::TransformDone { .. } => "transform_done",
                ProgressEvent::Done { .. } => "done",
                _ => "other",
            });
        }
        assert_eq!(events.first(), Some(&"started"));
        // pack() no longer emits the terminal `Done` event itself —
        // that's the app layer's responsibility, sent only after
        // `store_result` has stashed the PackResult so a fast renderer
        // can't race past it. The transform phase emits its own
        // start/done lifecycle events AFTER BuildingOutput (phase
        // boundary), so BuildingOutput is no longer strictly last —
        // assert ordering: building precedes the first transform event.
        assert!(!events.contains(&"done"));
        assert!(events.contains(&"building"));
        let building_idx = events.iter().position(|e| *e == "building").unwrap();
        let first_transform_idx = events.iter().position(|e| *e == "transform_start");
        if let Some(t_idx) = first_transform_idx {
            assert!(
                building_idx < t_idx,
                "BuildingOutput must precede the first TransformStart"
            );
        }
    }

    /// Repos with build artifacts (target/, node_modules, .git/objects) can
    /// produce tens of thousands of skipped paths. Emitting one IPC event per
    /// skipped file froze the UI for minutes — see commit history. Assert no
    /// per-file FileSkipped events are emitted regardless of skipped-count.
    #[test]
    fn does_not_flood_progress_with_per_file_skipped_events() {
        let d = tempdir().unwrap();
        // 50 files matching a custom-ignore pattern → walker.skipped contains 50 entries.
        for i in 0..50 {
            fs::write(d.path().join(format!("garbage_{i}.bin")), b"x").unwrap();
        }
        fs::write(d.path().join("keep.rs"), "fn main() {}\n").unwrap();

        let opts = PackOptions {
            goal: "x".into(),
            count_tokens: false,
            secret_scan: false,
            respect_gitignore: false,
            custom_ignore_patterns: vec!["garbage_*.bin".into()],
            ..PackOptions::default()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts,
            tx,
            "job-skip-flood",
            CancellationToken::new(), None,
        )
        .unwrap();
        let skipped_event_count = rx
            .try_iter()
            .filter(|e| matches!(e, ProgressEvent::FileSkipped { .. }))
            .count();
        assert_eq!(
            skipped_event_count, 0,
            "no per-file FileSkipped events should be emitted (got {skipped_event_count})"
        );
        // Skipped count is still preserved in the final stats.
        assert_eq!(result.stats.files_skipped, 50);
        assert_eq!(result.stats.files_included, 1);
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
        let result = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-test", CancellationToken::new(), None).unwrap();

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
    fn pack_populates_transform_phase_ms_field() {
        let d = fixture();
        let opts = PackOptions {
            goal: "x".into(),
            secret_scan: false,
            count_tokens: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(
            &PackTarget::Folder(d.path().to_path_buf()),
            &opts, tx, "job-test", CancellationToken::new(), None,
        ).unwrap();
        // Phase ran (even if reports vec is empty until individual transforms wire in).
        assert!(result.stats.transform_phase_ms <= result.stats.duration_ms,
            "transform_phase_ms ({}) must fit within duration_ms ({})",
            result.stats.transform_phase_ms, result.stats.duration_ms);
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
            &opts, tx, "job-test", CancellationToken::new(), None,
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
            &opts, tx, "job-test", CancellationToken::new(), None,
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
            &opts, tx, "job-test", CancellationToken::new(), None,
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
        let result = pack(&PackTarget::Folder(d.path().to_path_buf()), &opts, tx, "job-cancel-test", cancel, None);
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
            CancellationToken::new(), None,
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
            CancellationToken::new(), None,
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
            CancellationToken::new(), None,
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
            CancellationToken::new(), None,
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
            CancellationToken::new(), None,
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

    // ── EventThrottler unit tests (Stream 6) ─────────────────────────────────

    /// Walking events must be coalesced inside the throttler window: 100
    /// `push_walking` calls in a tight loop should result in strictly fewer
    /// than 100 sends because intermediate values are dropped.
    #[test]
    fn throttler_coalesces_walking_events() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut throttler = EventThrottler::new(tx);
        for i in 0..100u32 {
            throttler.push_walking(i);
        }
        // Ensure any pending walking value is emitted (and Drop runs).
        drop(throttler);
        let walking_count = rx
            .try_iter()
            .filter(|e| matches!(e, ProgressEvent::Walking { .. }))
            .count();
        assert!(
            walking_count < 100,
            "walking should be coalesced; got {walking_count} sends for 100 push_walking calls"
        );
        // At least one Walking event must have been emitted (the final flush).
        assert!(walking_count >= 1, "expected at least one Walking emission");
    }

    /// Buffer 51 FileFound items: the first 50 must flush as a single
    /// `FileFoundBatch`; the 51st remains buffered until `Drop` flushes it
    /// as a second batch with one entry.
    #[test]
    fn throttler_batches_file_found_at_50() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut throttler = EventThrottler::new(tx);
        for i in 0..51u32 {
            throttler.push_file_found(FileFound {
                path: format!("f{i}.rs"),
                bytes: 1,
            });
        }
        // Drop forces a flush of the trailing buffered item.
        drop(throttler);
        let batches: Vec<Vec<FileFound>> = rx
            .try_iter()
            .filter_map(|e| match e {
                ProgressEvent::FileFoundBatch { paths } => Some(paths),
                _ => None,
            })
            .collect();
        assert_eq!(
            batches.len(),
            2,
            "expected 1 full batch + 1 buffered (drop-flushed) = 2; got {batches:?}"
        );
        assert_eq!(batches[0].len(), 50, "first batch must hit the 50-item cap");
        assert_eq!(batches[1].len(), 1, "trailing batch must contain the 1 buffered item");
    }

    /// SecretHits emitted with a delay between groups should fire as
    /// per-group flushes, throttled to at most one flush per
    /// [`THROTTLE_WINDOW`]. Spacing the second group beyond the window
    /// proves the second flush is allowed.
    #[test]
    fn throttler_groups_secret_hits_per_100ms() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut throttler = EventThrottler::new(tx);

        // First group: pushed in a tight loop. The first push fires a flush
        // (no `last_secret_emit` yet); subsequent pushes stay buffered until
        // the next allowed flush.
        for i in 0..5u32 {
            throttler.push_secret_hit("a.rs".into(), "aws_access_key".into(), i);
        }

        // Wait beyond the throttle window before pushing the second group.
        std::thread::sleep(THROTTLE_WINDOW + Duration::from_millis(20));

        for i in 0..5u32 {
            throttler.push_secret_hit("b.rs".into(), "github_token".into(), i);
        }
        // Drop flushes any tail.
        drop(throttler);

        let hit_count = rx
            .try_iter()
            .filter(|e| matches!(e, ProgressEvent::SecretHit { .. }))
            .count();
        // All 10 hits must reach the wire (no drops). The grouping just
        // controls *when* they're flushed, not whether they're emitted.
        assert_eq!(
            hit_count, 10,
            "all 10 SecretHits must be emitted across grouped flushes"
        );
    }

    /// `flush_all` must flush in deterministic order (Walking first, then
    /// FileFoundBatch, then SecretHits) so the wire ordering is stable
    /// across phase transitions.
    #[test]
    fn throttler_flush_all_drains_all_buffers() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut throttler = EventThrottler::new(tx);
        throttler.push_walking(7);
        throttler.push_file_found(FileFound { path: "x.rs".into(), bytes: 1 });
        throttler.push_secret_hit("x.rs".into(), "rule".into(), 1);
        throttler.flush_all();
        drop(throttler);

        let events: Vec<ProgressEvent> = rx.try_iter().collect();
        let walking = events.iter().filter(|e| matches!(e, ProgressEvent::Walking { .. })).count();
        let batches = events.iter().filter(|e| matches!(e, ProgressEvent::FileFoundBatch { .. })).count();
        let hits = events.iter().filter(|e| matches!(e, ProgressEvent::SecretHit { .. })).count();
        assert!(walking >= 1, "walking must be flushed");
        assert!(batches >= 1, "file-found batch must be flushed");
        assert!(hits >= 1, "secret hits must be flushed");
    }

    /// `send_passthrough` must flush all queued throttled state before
    /// emitting the pass-through event so wire ordering is preserved.
    #[test]
    fn throttler_passthrough_flushes_first() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut throttler = EventThrottler::new(tx);
        throttler.push_file_found(FileFound { path: "x.rs".into(), bytes: 1 });
        throttler.send_passthrough(ProgressEvent::BuildingOutput);
        drop(throttler);

        let events: Vec<ProgressEvent> = rx.try_iter().collect();
        // Find the indexes of the FileFoundBatch and BuildingOutput. The
        // batch must arrive before BuildingOutput.
        let batch_idx = events
            .iter()
            .position(|e| matches!(e, ProgressEvent::FileFoundBatch { .. }))
            .expect("FileFoundBatch must be emitted before pass-through");
        let bo_idx = events
            .iter()
            .position(|e| matches!(e, ProgressEvent::BuildingOutput))
            .expect("BuildingOutput must be emitted");
        assert!(
            batch_idx < bo_idx,
            "FileFoundBatch must precede BuildingOutput (got batch_idx={batch_idx}, bo_idx={bo_idx})"
        );
    }
}
