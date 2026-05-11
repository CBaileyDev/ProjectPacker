//! Pack-content compression transforms. See
//! `docs/superpowers/specs/2026-05-11-v06-lossless-compression-design.md`.

use crate::pack::FileEntry;
use crate::types::{PackOptions, TransformReport};
use std::time::Instant;

/// Run every enabled transform over `entries` in fixed order.
/// Returns the per-transform reports and total phase elapsed in ms.
pub fn run_transform_phase(
    entries: &mut [FileEntry],
    opts: &PackOptions,
) -> (Vec<TransformReport>, u32) {
    let start = Instant::now();
    let reports: Vec<TransformReport> = Vec::new();
    // Individual transforms are wired in subsequent tasks.
    let _ = entries;
    let _ = opts;
    let elapsed = start.elapsed().as_millis() as u32;
    (reports, elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::FileEntry;

    #[test]
    fn empty_pipeline_is_a_no_op() {
        let mut entries = vec![FileEntry {
            path: "a.rs".into(),
            content: "fn x() {}\n".into(),
            bytes: 11,
            tokens: None,
            hash: "deadbeef".into(),
        }];
        let original = entries[0].content.clone();
        let opts = PackOptions::default();
        let (reports, _ms) = run_transform_phase(&mut entries, &opts);
        assert!(reports.is_empty());
        assert_eq!(entries[0].content, original);
    }
}
