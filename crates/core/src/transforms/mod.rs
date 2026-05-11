//! Pack-content compression transforms. See
//! `docs/superpowers/specs/2026-05-11-v06-lossless-compression-design.md`.

use crate::pack::FileEntry;
use crate::types::{PackOptions, ProgressEvent, TransformReport};
use std::sync::mpsc::Sender;
use std::time::Instant;

pub mod collapse_lockfile;
pub mod collapse_minified;
pub mod dedup;
pub mod normalize;

/// Run every enabled transform over `entries` in fixed order, emitting
/// `TransformStart`/`TransformDone` events for each ENABLED transform.
/// Returns reports + total phase elapsed (ms).
///
/// `tx` is the unbottled event channel — these events are pass-through and
/// don't go through the orchestrator's throttler (low frequency, 10 max).
pub fn run_transform_phase(
    entries: &mut [FileEntry],
    opts: &PackOptions,
    tx: &Sender<ProgressEvent>,
) -> (Vec<TransformReport>, u32) {
    let phase_start = Instant::now();
    let mut reports: Vec<TransformReport> = Vec::new();

    // ── Order: cheapest per-file, then cross-file dedup, then semantic, then lossy.
    if opts.trim_trailing_ws {
        let _ = tx.send(ProgressEvent::TransformStart { id: "trim_trailing_ws".into() });
        let r = per_file(entries, "trim_trailing_ws", normalize::trim_trailing_ws);
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    if opts.collapse_blank_lines {
        let _ = tx.send(ProgressEvent::TransformStart { id: "collapse_blank_lines".into() });
        let r = per_file(entries, "collapse_blank_lines", normalize::collapse_blank_lines);
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    if opts.normalize_line_endings {
        let _ = tx.send(ProgressEvent::TransformStart { id: "normalize_line_endings".into() });
        let r = per_file(entries, "normalize_line_endings", normalize::normalize_line_endings);
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    if opts.dedup_files {
        let _ = tx.send(ProgressEvent::TransformStart { id: "dedup_files".into() });
        let r = dedup::apply(entries);
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    // Semantic + lossy transforms wire in subsequent tasks.

    let elapsed = phase_start.elapsed().as_millis() as u32;
    (reports, elapsed)
}

/// Apply a per-file fn returning Option<String> (None means unchanged) over
/// every entry in parallel. Sums savings into a TransformReport.
fn per_file(
    entries: &mut [FileEntry],
    id: &str,
    f: fn(&str) -> Option<String>,
) -> TransformReport {
    use rayon::prelude::*;
    let start = Instant::now();
    let changes: Vec<(usize, String, u64)> = entries
        .par_iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let before = e.content.len() as u64;
            let new_content = f(&e.content)?;
            let saved = before.saturating_sub(new_content.len() as u64);
            Some((i, new_content, saved))
        })
        .collect();
    let files_touched = changes.len() as u32;
    let mut bytes_saved = 0u64;
    for (i, new_content, saved) in changes {
        entries[i].content = new_content;
        bytes_saved = bytes_saved.saturating_add(saved);
    }
    TransformReport {
        id: id.into(),
        bytes_saved,
        files_touched,
        elapsed_ms: start.elapsed().as_millis() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn entry(path: &str, content: &str, hash: &str) -> FileEntry {
        FileEntry {
            path: path.into(), content: content.into(), bytes: content.len() as u64,
            tokens: None, hash: hash.into(),
        }
    }

    #[test]
    fn empty_pipeline_is_a_no_op() {
        let mut entries = vec![entry("a.rs", "fn x() {}\n", "deadbeef")];
        let opts = PackOptions { // turn everything off
            dedup_files: false, trim_trailing_ws: false,
            collapse_blank_lines: false, normalize_line_endings: false,
            ..PackOptions::default()
        };
        let (tx, _rx) = mpsc::channel();
        let (reports, _) = run_transform_phase(&mut entries, &opts, &tx);
        assert!(reports.is_empty());
    }

    #[test]
    fn default_lossless_pipeline_emits_4_reports() {
        let mut entries = vec![
            entry("a.txt", "trail   \n\n\n\nthing\r\n", "h1"),
        ];
        let opts = PackOptions::default(); // 4 lossless ON
        let (tx, _rx) = mpsc::channel();
        let (reports, _) = run_transform_phase(&mut entries, &opts, &tx);
        let ids: Vec<&str> = reports.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec![
            "trim_trailing_ws",
            "collapse_blank_lines",
            "normalize_line_endings",
            "dedup_files",
        ]);
    }
}
