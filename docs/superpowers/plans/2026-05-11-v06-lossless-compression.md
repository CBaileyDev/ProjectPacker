# v0.6 Lossless Compression Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship 10 individually-toggleable compression transforms — 4 lossless ON by default, 3 semantic + 3 lossy OFF by default — with per-transform telemetry surfaced in a new grouped UI panel.

**Architecture:** Insert a new `transform` phase between `process` and `pin-reorder` in `crates/core/src/pack/orchestrator.rs`. Each transform is its own module file under a new `crates/core/src/transforms/` directory and produces a `TransformReport`. UI gets a `CompressionPanel` collapsible disclosure with three grouped sections.

**Tech Stack:** Rust 1.x, Tauri 2, tree-sitter, BLAKE3, rayon, React 19, Tailwind v4, Zustand 5, Vitest.

**Spec:** `docs/superpowers/specs/2026-05-11-v06-lossless-compression-design.md`

---

## File structure

### Created
- `crates/core/src/transforms/mod.rs` — phase entry + types
- `crates/core/src/transforms/dedup.rs`
- `crates/core/src/transforms/normalize.rs`
- `crates/core/src/transforms/collapse_lockfile.rs`
- `crates/core/src/transforms/collapse_minified.rs`
- `crates/core/src/transforms/mark_generated.rs`
- `crates/core/src/transforms/compress_skeleton.rs`
- `crates/core/src/transforms/strip_comments.rs`
- `crates/core/src/transforms/elide_types.rs`
- `tests/fixtures/compression/` (LICENSE x2, package-lock.json, bundle.min.js, generated.gen.ts, src.rs)
- `frontend/src/components/pack/CompressionPanel.tsx`
- `frontend/src/components/pack/TransformRow.tsx`
- `frontend/src/components/pack/CompressionPanel.test.tsx`

### Modified
- `crates/core/src/types.rs` — `PackOptions` (+8 fields), `TransformReport` (new), `PackStats` (+2 fields), `ProgressEvent` (+2 variants), `WarningKind` (+1 variant)
- `crates/core/src/lib.rs` — `pub mod transforms;`
- `crates/core/src/pack/orchestrator.rs` — call `run_transform_phase`
- `crates/core/src/pack/xml.rs` — `duplicate-of`/`sha` attrs + `<compression_report>` block
- `crates/core/src/pack/markdown.rs` — compression-report section
- `crates/core/src/pack/plain.rs` — compression-report section
- `crates/core/tests/pack_integration.rs` — new end-to-end tests
- `crates/core/tests/protocol_golden.rs` — new snapshot
- `frontend/src/routes/Pack.tsx` — mount `CompressionPanel`, remove old inline toggles
- `frontend/src/lib/store.ts` — extend Zustand store
- `frontend/src/bindings/index.ts` — regenerated
- `frontend/src/styles/globals.css` — new theme token
- `docs/protocol/grok-to-cc-v1.md` — markers section
- `CHANGELOG.md` — Unreleased entry

---

# Phase A — Pipeline plumbing

### Task A1: Extend types.rs with new transform + event types

**Files:**
- Modify: `crates/core/src/types.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `types.rs`:

```rust
#[test]
fn pack_options_deserializes_with_missing_new_fields() {
    // Simulates an old preset saved before v0.6 — no transform fields present.
    let json = r#"{
        "goal": "x",
        "format": "Xml",
        "xml_schema": "Cxml",
        "respect_gitignore": true,
        "secret_scan": true,
        "count_tokens": true,
        "compress": false,
        "remove_comments": false,
        "tokenizer_model": "claude",
        "protocol_version": "grok-to-cc-v1",
        "max_file_size_kb": 512,
        "custom_ignore_patterns": []
    }"#;
    let opts: PackOptions = serde_json::from_str(json).expect("old preset must deserialize");
    // The 4 lossless transforms default to ON.
    assert!(opts.dedup_files);
    assert!(opts.trim_trailing_ws);
    assert!(opts.collapse_blank_lines);
    assert!(opts.normalize_line_endings);
    // The 4 semantic+lossy transforms default to OFF.
    assert!(!opts.collapse_lockfiles);
    assert!(!opts.collapse_minified);
    assert!(!opts.mark_generated);
    assert!(!opts.elide_type_only_exports);
}

#[test]
fn transform_report_round_trips_via_serde() {
    let r = TransformReport {
        id: "dedup_files".into(),
        bytes_saved: 1234,
        files_touched: 2,
        elapsed_ms: 7,
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: TransformReport = serde_json::from_str(&s).unwrap();
    assert_eq!(back.id, "dedup_files");
    assert_eq!(back.bytes_saved, 1234);
    assert_eq!(back.files_touched, 2);
    assert_eq!(back.elapsed_ms, 7);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p projectpacker-core pack_options_deserializes_with_missing_new_fields transform_report_round_trips_via_serde --tests`
Expected: FAIL — fields/types don't exist yet.

- [ ] **Step 3: Add the new types and fields**

In `crates/core/src/types.rs`, add these helpers and types (place near other shared helpers; keep `serde` derives consistent with existing types — `Serialize, Deserialize, specta::Type`):

```rust
fn default_true() -> bool { true }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransformReport {
    pub id: String,
    pub bytes_saved: u64,
    pub files_touched: u32,
    pub elapsed_ms: u32,
}
```

Extend the existing `PackOptions` struct with 8 new fields (preserve existing serde renames/order). Add after `remove_comments`:

```rust
    #[serde(default = "default_true")]
    pub dedup_files: bool,
    #[serde(default = "default_true")]
    pub trim_trailing_ws: bool,
    #[serde(default = "default_true")]
    pub collapse_blank_lines: bool,
    #[serde(default = "default_true")]
    pub normalize_line_endings: bool,
    #[serde(default)]
    pub collapse_lockfiles: bool,
    #[serde(default)]
    pub collapse_minified: bool,
    #[serde(default)]
    pub mark_generated: bool,
    #[serde(default)]
    pub elide_type_only_exports: bool,
```

Update `impl Default for PackOptions` (locate the existing impl and add the same 8 fields with matching default values).

Extend the existing `PackStats` struct with two new fields (at the end, before the closing brace):

```rust
    pub transforms: Vec<TransformReport>,
    pub transform_phase_ms: u32,
```

Extend `ProgressEvent` enum with two new variants (keep existing camelCase / tag-rename conventions):

```rust
    TransformStart { id: String },
    TransformDone { id: String, bytes_saved: u64, files_touched: u32 },
```

Extend `WarningKind` enum with one new variant (preserve the existing serde rename style):

```rust
    TransformFailed,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p projectpacker-core pack_options_deserializes_with_missing_new_fields transform_report_round_trips_via_serde --tests`
Expected: PASS.

- [ ] **Step 5: Run the full test suite — there should be exhaustive-match breakages**

Run: `cargo test --workspace --tests`
Expected: COMPILE FAILURES in `pack/xml.rs`, `pack/markdown.rs`, `pack/plain.rs`, `commands.rs`, or anywhere else `match` arms cover `ProgressEvent` / `WarningKind` exhaustively. **Fix each one** by adding the new arms with placeholder behavior:

- For `ProgressEvent::TransformStart` / `TransformDone` in renderer match arms: ignore (return early). They don't appear in pack output.
- For `WarningKind::TransformFailed` in renderer/display code: format as `"transform failed"`.
- Update `PackStats { … }` literal constructions across the codebase to include `transforms: Vec::new(), transform_phase_ms: 0`.
- Update `PackOptions { … }` literal constructions (test fixtures, etc.) to include the 8 new fields with default values (true for lossless, false for semantic/lossy).

Re-run until clean.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/types.rs crates/core/src/pack/ crates/app/src/
git commit -m "$(cat <<'EOF'
feat(core): add TransformReport + PackOptions transform toggles

8 new PackOptions fields (4 lossless default-ON, 4 opt-in default-OFF) with
serde defaults so old presets deserialize cleanly. New TransformReport type,
PackStats.transforms/transform_phase_ms, ProgressEvent::TransformStart/Done,
WarningKind::TransformFailed. No behavior change yet — wiring only.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task A2: Create empty transforms module

**Files:**
- Create: `crates/core/src/transforms/mod.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/transforms/mod.rs` with a placeholder test:

```rust
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
```

In `crates/core/src/lib.rs`, add (after other `pub mod` declarations, alphabetical order):

```rust
pub mod transforms;
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p projectpacker-core transforms::tests::empty_pipeline_is_a_no_op --tests`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/transforms/mod.rs crates/core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(core): scaffold transforms module with empty pipeline

run_transform_phase signature locked in (mutates entries, returns reports +
elapsed ms). Individual transforms wired in subsequent tasks.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task A3: Wire transform phase into orchestrator

**Files:**
- Modify: `crates/core/src/pack/orchestrator.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `orchestrator.rs`:

```rust
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
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p projectpacker-core pack_populates_transform_phase_ms_field --tests`
Expected: FAIL — `transform_phase_ms` not populated.

- [ ] **Step 3: Wire the phase into `pack()`**

In `crates/core/src/pack/orchestrator.rs`, locate `pack()`. After `apply_pin_reorder(...)` and BEFORE `run_secret_scan_phase(...)`, insert:

```rust
    // Checkpoint: between pin-reorder and secret-scan.
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    let (transform_reports, transform_phase_ms) =
        crate::transforms::run_transform_phase(&mut entries, opts);
```

Then in the `PackStats` first construction further down, replace:

```rust
        emit_ms: 0,
    };
```

with:

```rust
        emit_ms: 0,
        transforms: transform_reports.clone(),
        transform_phase_ms,
    };
```

And in the refreshed stats:

```rust
    let stats = PackStats {
        emit_ms,
        duration_ms: start.elapsed().as_millis() as u32,
        ..stats
    };
```

(No change to the refresh — `..stats` already carries the new fields through.)

Also add `use crate::transforms;` import at the top of the file if not present.

- [ ] **Step 4: Run the test**

Run: `cargo test -p projectpacker-core pack_populates_transform_phase_ms_field --tests`
Expected: PASS.

- [ ] **Step 5: Verify the full test suite still passes**

Run: `cargo test --workspace --tests`
Expected: 250+ passing, 0 failures.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/pack/orchestrator.rs
git commit -m "$(cat <<'EOF'
feat(core): insert transform phase into pack orchestrator

Phase runs between pin-reorder and secret-scan (lossless transforms before
secret-scan so redaction byte offsets align with the transformed content).
Empty reports vec for now; individual transforms wire in next phase.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task A4: Regenerate TS bindings

**Files:**
- Modify: `frontend/src/bindings/index.ts` (regenerated)

- [ ] **Step 1: Regenerate**

Run: `pnpm bindings`
(equivalently: `cargo run -p projectpacker-app --bin emit-bindings`)
Expected: `frontend/src/bindings/index.ts` updates with the new types: `TransformReport`, `PackOptions` fields, `PackStats` fields, `ProgressEvent::TransformStart`/`TransformDone`, `WarningKind::TransformFailed`.

- [ ] **Step 2: Verify frontend typecheck still passes**

Run: `pnpm --dir frontend typecheck`
Expected: 0 errors. The new types are unused in the frontend yet — that's fine.

- [ ] **Step 3: Commit**

```bash
git add frontend/src/bindings/index.ts
git commit -m "$(cat <<'EOF'
chore(bindings): regenerate after transform types added

Adds TransformReport, 8 new PackOptions fields, PackStats.transforms +
transform_phase_ms, TransformStart/Done events, TransformFailed warning kind.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase B — Lossless 4

### Task B1: Implement normalize.rs (3 transforms)

**Files:**
- Create: `crates/core/src/transforms/normalize.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/transforms/normalize.rs`:

```rust
//! Three lossless per-line normalization transforms:
//! `trim_trailing_ws`, `collapse_blank_lines`, `normalize_line_endings`.

/// Strip trailing spaces and tabs from each line. Returns `Some(new)` if any
/// changes were made, `None` if the input was already normalized (so callers
/// can skip allocation in the common case).
pub fn trim_trailing_ws(s: &str) -> Option<String> {
    if !s.lines().any(|l| l.ends_with(' ') || l.ends_with('\t')) {
        return None;
    }
    let trailing_nl = s.ends_with('\n');
    let mut out: String = s
        .lines()
        .map(|l| l.trim_end_matches(|c: char| c == ' ' || c == '\t'))
        .collect::<Vec<_>>()
        .join("\n");
    if trailing_nl {
        out.push('\n');
    }
    Some(out)
}

/// Runs of >=3 blank lines collapse to exactly 2. Single & double blank-line
/// separation preserved. Returns `Some(new)` only if anything changed.
pub fn collapse_blank_lines(s: &str) -> Option<String> {
    // Quick scan for a run of 3 consecutive blank lines.
    let mut run = 0usize;
    let mut needs_collapse = false;
    for l in s.lines() {
        if l.trim().is_empty() {
            run += 1;
            if run >= 3 { needs_collapse = true; break; }
        } else {
            run = 0;
        }
    }
    if !needs_collapse { return None; }

    let trailing_nl = s.ends_with('\n');
    let mut out_lines: Vec<&str> = Vec::with_capacity(s.lines().count());
    let mut blank_run = 0usize;
    for l in s.lines() {
        if l.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 2 { out_lines.push(l); }
            // skip lines past the second blank
        } else {
            blank_run = 0;
            out_lines.push(l);
        }
    }
    let mut out = out_lines.join("\n");
    if trailing_nl { out.push('\n'); }
    Some(out)
}

/// CRLF → LF and lone CR → LF. Idempotent. Returns `Some(new)` only if
/// anything changed.
pub fn normalize_line_endings(s: &str) -> Option<String> {
    if !s.contains('\r') { return None; }
    let step1 = s.replace("\r\n", "\n");
    let out = step1.replace('\r', "\n");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_strips_trailing_spaces_and_tabs() {
        let input = "foo   \nbar\t\nbaz\n";
        let out = trim_trailing_ws(input).expect("should change");
        assert_eq!(out, "foo\nbar\nbaz\n");
    }

    #[test]
    fn trim_returns_none_when_already_clean() {
        let input = "foo\nbar\n";
        assert!(trim_trailing_ws(input).is_none());
    }

    #[test]
    fn trim_preserves_intentional_leading_indentation() {
        let input = "  indented   \n    deeper  \n";
        let out = trim_trailing_ws(input).expect("should change");
        assert_eq!(out, "  indented\n    deeper\n");
    }

    #[test]
    fn collapse_blank_collapses_3plus_to_2() {
        let input = "a\n\n\n\nb\n";
        let out = collapse_blank_lines(input).expect("should change");
        assert_eq!(out, "a\n\n\nb\n");
    }

    #[test]
    fn collapse_blank_preserves_single_and_double_blanks() {
        let input = "a\n\nb\n\n\nc\n";
        let out = collapse_blank_lines(input).expect("should change");
        // single blank preserved; triple → double
        assert_eq!(out, "a\n\nb\n\n\nc\n");
    }

    #[test]
    fn collapse_blank_returns_none_when_no_runs() {
        let input = "a\n\nb\nc\n";
        assert!(collapse_blank_lines(input).is_none());
    }

    #[test]
    fn normalize_line_endings_converts_crlf() {
        let input = "a\r\nb\r\nc\n";
        let out = normalize_line_endings(input).expect("should change");
        assert_eq!(out, "a\nb\nc\n");
    }

    #[test]
    fn normalize_line_endings_converts_lone_cr() {
        let input = "old\rmac\rfile\n";
        let out = normalize_line_endings(input).expect("should change");
        assert_eq!(out, "old\nmac\nfile\n");
    }

    #[test]
    fn normalize_line_endings_returns_none_for_lf_only() {
        let input = "a\nb\nc\n";
        assert!(normalize_line_endings(input).is_none());
    }

    #[test]
    fn all_three_are_idempotent() {
        let input = "a   \n\n\n\nb\r\n";
        let after_trim = trim_trailing_ws(input).unwrap();
        assert!(trim_trailing_ws(&after_trim).is_none(), "trim must be idempotent");
        let after_blank = collapse_blank_lines(&after_trim).unwrap();
        assert!(collapse_blank_lines(&after_blank).is_none(), "collapse must be idempotent");
        let after_eol = normalize_line_endings(&after_blank).unwrap();
        assert!(normalize_line_endings(&after_eol).is_none(), "EOL normalize must be idempotent");
    }
}
```

- [ ] **Step 2: Wire into mod.rs**

In `crates/core/src/transforms/mod.rs`, add at the top:

```rust
pub mod normalize;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p projectpacker-core transforms::normalize --tests`
Expected: 10/10 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/transforms/normalize.rs crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): add normalize module (trim_ws, blanks, line endings)

Three pure-fn transforms. Each returns Option<String>: None when input
is already normalized (lets callers skip allocation in the common case).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task B2: Implement dedup.rs (cross-file)

**Files:**
- Create: `crates/core/src/transforms/dedup.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/transforms/dedup.rs`:

```rust
//! Cross-file deduplication via BLAKE3 hash grouping.

use crate::pack::FileEntry;
use crate::types::TransformReport;
use std::collections::HashMap;
use std::time::Instant;

/// For each group of files sharing a `hash`, the lexicographically-first path
/// keeps content; the rest get replaced with a marker referencing the first.
/// Mutates `entries` in place. Returns the report.
pub fn apply(entries: &mut [FileEntry]) -> TransformReport {
    let start = Instant::now();
    // Group indices by hash.
    let mut by_hash: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        // Skip empty hashes (failed reads).
        if e.hash.is_empty() { continue; }
        by_hash.entry(e.hash.as_str()).or_default().push(i);
    }
    // Collect groups with >1 member; lift index refs out before mutating.
    let dup_groups: Vec<Vec<usize>> = by_hash
        .into_iter()
        .filter_map(|(_, idxs)| if idxs.len() > 1 { Some(idxs) } else { None })
        .collect();
    let mut bytes_saved: u64 = 0;
    let mut files_touched: u32 = 0;
    for group in dup_groups {
        // Find the path-lexicographic-min in the group.
        let mut sorted = group.clone();
        sorted.sort_by(|&a, &b| entries[a].path.cmp(&entries[b].path));
        let first_idx = sorted[0];
        let first_path = entries[first_idx].path.clone();
        let first_hash_prefix: String = entries[first_idx].hash.chars().take(12).collect();
        for &idx in sorted.iter().skip(1) {
            // Marker replaces the content; bytes_saved is the original byte count we no longer emit.
            let original_len = entries[idx].content.len() as u64;
            entries[idx].content = format!(
                "[DUPLICATE OF: {first_path} | sha: {first_hash_prefix}]\n"
            );
            // bytes_saved: original minus the marker we now emit.
            let new_len = entries[idx].content.len() as u64;
            bytes_saved = bytes_saved.saturating_add(original_len.saturating_sub(new_len));
            files_touched += 1;
        }
    }
    TransformReport {
        id: "dedup_files".into(),
        bytes_saved,
        files_touched,
        elapsed_ms: start.elapsed().as_millis() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, content: &str, hash: &str) -> FileEntry {
        FileEntry {
            path: path.into(),
            content: content.into(),
            bytes: content.len() as u64,
            tokens: None,
            hash: hash.into(),
        }
    }

    #[test]
    fn two_identical_files_keep_first_replace_second() {
        let body = "Apache License 2.0\n... full license body ...\n";
        let mut entries = vec![
            entry("LICENSE", body, "ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890"),
            entry("vendor/copy/LICENSE", body, "ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890ab12cd34ef567890"),
        ];
        let report = apply(&mut entries);
        assert_eq!(report.id, "dedup_files");
        assert_eq!(report.files_touched, 1);
        assert!(report.bytes_saved > 0);
        assert_eq!(entries[0].content, body, "first occurrence preserved");
        assert!(entries[1].content.starts_with("[DUPLICATE OF: LICENSE"));
        assert!(entries[1].content.contains("sha: ab12cd34ef56"));
    }

    #[test]
    fn three_identical_files_keep_first_replace_others() {
        let body = "x".repeat(500);
        let mut entries = vec![
            entry("c.txt", &body, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
            entry("a.txt", &body, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
            entry("b.txt", &body, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
        ];
        let report = apply(&mut entries);
        assert_eq!(report.files_touched, 2);
        // a.txt is the lexicographic minimum and should retain its content.
        let a = entries.iter().find(|e| e.path == "a.txt").unwrap();
        assert_eq!(a.content, body);
        let b = entries.iter().find(|e| e.path == "b.txt").unwrap();
        assert!(b.content.starts_with("[DUPLICATE OF: a.txt"));
        let c = entries.iter().find(|e| e.path == "c.txt").unwrap();
        assert!(c.content.starts_with("[DUPLICATE OF: a.txt"));
    }

    #[test]
    fn singletons_left_alone() {
        let mut entries = vec![
            entry("a.txt", "foo", "1111111111111111111111111111111111111111111111111111111111111111"),
            entry("b.txt", "bar", "2222222222222222222222222222222222222222222222222222222222222222"),
        ];
        let original = entries.clone();
        let report = apply(&mut entries);
        assert_eq!(report.files_touched, 0);
        assert_eq!(report.bytes_saved, 0);
        assert_eq!(entries, original);
    }

    #[test]
    fn empty_hashes_skipped() {
        let mut entries = vec![
            entry("a.txt", "", ""),
            entry("b.txt", "", ""),
        ];
        let report = apply(&mut entries);
        // No dedup against empty hashes — both kept untouched.
        assert_eq!(report.files_touched, 0);
    }

    #[test]
    fn sort_stability_path_lex_min_wins_regardless_of_input_order() {
        let body = "same\n";
        let hash = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        // Walker emits zzz first, but aaa should win the sort.
        let mut entries = vec![
            entry("zzz.txt", body, hash),
            entry("aaa.txt", body, hash),
        ];
        apply(&mut entries);
        let aaa = entries.iter().find(|e| e.path == "aaa.txt").unwrap();
        let zzz = entries.iter().find(|e| e.path == "zzz.txt").unwrap();
        assert_eq!(aaa.content, body, "lex-min path wins regardless of walker order");
        assert!(zzz.content.starts_with("[DUPLICATE OF: aaa.txt"));
    }
}
```

- [ ] **Step 2: Wire into mod.rs**

In `crates/core/src/transforms/mod.rs`:

```rust
pub mod dedup;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p projectpacker-core transforms::dedup --tests`
Expected: 5/5 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/transforms/dedup.rs crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): add cross-file dedup using existing BLAKE3 hashes

Lex-min path within each hash group retains content; others get a marker.
Zero new computation — hash is already populated by run_process_phase.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task B3: Wire lossless transforms into the pipeline

**Files:**
- Modify: `crates/core/src/transforms/mod.rs`
- Modify: `crates/core/src/pack/orchestrator.rs` (event throttler integration)

- [ ] **Step 1: Update run_transform_phase**

Replace the body of `run_transform_phase` in `crates/core/src/transforms/mod.rs` with:

```rust
use crate::pack::FileEntry;
use crate::types::{PackOptions, ProgressEvent, TransformReport};
use std::sync::mpsc::Sender;
use std::time::Instant;

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
```

- [ ] **Step 2: Update the orchestrator call site**

In `crates/core/src/pack/orchestrator.rs`, find the line you added in Task A3:

```rust
    let (transform_reports, transform_phase_ms) =
        crate::transforms::run_transform_phase(&mut entries, opts);
```

Replace with:

```rust
    let (transform_reports, transform_phase_ms) =
        crate::transforms::run_transform_phase(&mut entries, opts, throttler.passthrough_tx());
```

Now add a `passthrough_tx()` accessor on `EventThrottler` in the same file. Find the `impl EventThrottler` block and add (after `fn new`):

```rust
    /// Borrow the underlying tx for low-frequency pass-through events
    /// (transform lifecycle, etc.). The throttler's own buffers are NOT
    /// flushed first — callers must ensure that's correct for their event.
    /// For TransformStart/Done this is safe: those events have no ordering
    /// constraint with FileFound or SecretHit.
    fn passthrough_tx(&self) -> &Sender<ProgressEvent> {
        &self.tx
    }
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p projectpacker-core transforms --tests`
Expected: PASS (existing + 2 new mod tests).
Run: `cargo test --workspace --tests`
Expected: still all passing.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/transforms/mod.rs crates/core/src/pack/orchestrator.rs
git commit -m "$(cat <<'EOF'
feat(transforms): wire 4 lossless transforms into the pipeline

trim_trailing_ws, collapse_blank_lines, normalize_line_endings, dedup_files
all run by default. Per-file transforms parallelize via rayon; dedup is a
single-threaded cross-file pass over the existing BLAKE3 hashes.

Each enabled transform emits TransformStart/TransformDone events for live
UI updates.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase C — Semantic 3

### Task C1: Implement collapse_lockfile.rs

**Files:**
- Create: `crates/core/src/transforms/collapse_lockfile.rs`

- [ ] **Step 1: Write the failing tests**

Create the file with:

```rust
//! Lockfile detection + body collapse.

const LOCKFILE_BASENAMES: &[&str] = &[
    "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "Cargo.lock",
    "Gemfile.lock", "poetry.lock", "composer.lock", "Pipfile.lock", "go.sum",
];

pub fn is_lockfile(path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path);
    let basename = basename.rsplit('\\').next().unwrap_or(basename);
    LOCKFILE_BASENAMES.iter().any(|&n| n == basename)
}

/// Returns `Some(collapsed_body)` if `path` is a lockfile, else `None`.
/// `hash_prefix` is the first 12 chars of the file's BLAKE3 hash.
pub fn collapse(path: &str, content: &str, hash_prefix: &str) -> Option<String> {
    if !is_lockfile(path) { return None; }
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total <= 25 { return None; } // short lockfile — keep as-is
    let head: String = lines.iter().take(20).copied().collect::<Vec<_>>().join("\n");
    let tail: String = lines.iter().rev().take(5).rev().copied().collect::<Vec<_>>().join("\n");
    let omitted = total.saturating_sub(25);
    let original_bytes = content.len();
    Some(format!(
        "{head}\n[COMPRESSED: lockfile | original-bytes: {original_bytes} | sha: {hash_prefix}]\n[{omitted} lines omitted]\n{tail}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_lockfile_basenames() {
        assert!(is_lockfile("package-lock.json"));
        assert!(is_lockfile("apps/foo/pnpm-lock.yaml"));
        assert!(is_lockfile("crates\\Cargo.lock"));
        assert!(!is_lockfile("src/main.rs"));
    }

    #[test]
    fn collapse_keeps_short_lockfile_intact() {
        let content = "{\n  \"name\": \"x\"\n}\n";
        assert!(collapse("package-lock.json", content, "deadbeef0000").is_none());
    }

    #[test]
    fn collapse_replaces_long_lockfile_body() {
        let mut content = String::new();
        for i in 0..200 {
            content.push_str(&format!("  \"dep{i}\": \"1.0.0\"\n"));
        }
        let out = collapse("package-lock.json", &content, "deadbeef0000").unwrap();
        assert!(out.starts_with("  \"dep0\""), "preserves first 20 lines");
        assert!(out.contains("[COMPRESSED: lockfile"));
        assert!(out.contains("175 lines omitted"));
        assert!(out.contains("\"dep199\""), "preserves last 5 lines");
    }

    #[test]
    fn collapse_returns_none_for_non_lockfile() {
        let content = "fn main() {}\n".repeat(50);
        assert!(collapse("src/main.rs", &content, "abc").is_none());
    }
}
```

In `crates/core/src/transforms/mod.rs`, add:

```rust
pub mod collapse_lockfile;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p projectpacker-core transforms::collapse_lockfile --tests`
Expected: 4/4 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/transforms/collapse_lockfile.rs crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): add lockfile detection + body collapse

9 known lockfile basenames (npm/pnpm/yarn/cargo/ruby/python/php/go).
Keeps first 20 + last 5 lines; replaces middle with a SHA-pinned marker.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task C2: Implement collapse_minified.rs

**Files:**
- Create: `crates/core/src/transforms/collapse_minified.rs`

- [ ] **Step 1: Write the failing tests**

Create the file with:

```rust
//! Minified-bundle detection + body collapse.

const LONG_LINE_THRESHOLD: usize = 2000;
const AVG_OVER_MEDIAN_RATIO: f64 = 5.0;

pub fn is_minified(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() { return false; }
    let has_long = lines.iter().any(|l| l.len() > LONG_LINE_THRESHOLD);
    if !has_long { return false; }
    // Single-line files: long line alone is sufficient.
    if lines.len() <= 5 { return true; }
    // Multi-line: check avg/median ratio to suppress tab-aligned data files.
    let mut lens: Vec<usize> = lines.iter().map(|l| l.len()).collect();
    lens.sort_unstable();
    let median = lens[lens.len() / 2].max(1);
    let avg = (lens.iter().sum::<usize>() as f64) / (lens.len() as f64);
    (avg / median as f64) > AVG_OVER_MEDIAN_RATIO
}

/// Returns `Some(collapsed)` if `content` looks minified, else `None`.
pub fn collapse(content: &str, hash_prefix: &str) -> Option<String> {
    if !is_minified(content) { return None; }
    let n = content.len();
    let head: String = content.chars().take(200).collect();
    let tail: String = content.chars().rev().take(100).collect::<String>()
        .chars().rev().collect();
    Some(format!(
        "{head}\n[MINIFIED BUNDLE: {n} bytes | sha: {hash_prefix}]\n{tail}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_single_line_minified_bundle() {
        let content: String = "a=1;".repeat(1000); // ~4000 chars, 1 line
        assert!(is_minified(&content));
    }

    #[test]
    fn does_not_detect_normal_source() {
        let content = "fn main() {\n    println!(\"hi\");\n}\n".repeat(20);
        assert!(!is_minified(&content));
    }

    #[test]
    fn does_not_detect_data_file_with_consistent_long_lines() {
        // 100 lines each 2100 chars — long, but consistent length.
        let line = "x".repeat(2100);
        let content = (0..100).map(|_| line.clone()).collect::<Vec<_>>().join("\n");
        // avg ≈ median, ratio ≈ 1, should NOT trip the heuristic.
        assert!(!is_minified(&content));
    }

    #[test]
    fn detects_mixed_short_lines_plus_one_huge_line() {
        // 50 short lines (10 chars) + 1 huge line (5000 chars).
        // avg = (50*10 + 5000) / 51 ≈ 108; median = 10; ratio ≈ 11 → trip.
        let mut content = String::new();
        for _ in 0..50 { content.push_str("short line\n"); }
        content.push_str(&"x".repeat(5000));
        content.push('\n');
        assert!(is_minified(&content));
    }

    #[test]
    fn collapse_replaces_minified_body() {
        let content: String = "var x=".repeat(500); // ~3000 chars, 1 line, no \n
        let out = collapse(&content, "abc123def456").unwrap();
        assert!(out.contains("[MINIFIED BUNDLE:"));
        assert!(out.contains("sha: abc123def456"));
        assert!(out.contains(&content[..200])); // head preserved
    }

    #[test]
    fn collapse_returns_none_for_non_minified() {
        let content = "regular\nsource\ncode\n";
        assert!(collapse(content, "abc").is_none());
    }
}
```

In `mod.rs`:

```rust
pub mod collapse_minified;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p projectpacker-core transforms::collapse_minified --tests`
Expected: 6/6 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/transforms/collapse_minified.rs crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): add minified-bundle detection + body collapse

Two-signal detection (long line + avg/median ratio) to avoid false
positives on tab-aligned data files. Single-line files with a long
line trip directly. Keeps first 200 + last 100 chars.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task C3: Implement mark_generated.rs

**Files:**
- Create: `crates/core/src/transforms/mark_generated.rs`

- [ ] **Step 1: Write the failing tests**

Create the file with:

```rust
//! Generated-file detection (banner scan + filename patterns) + body collapse.

const BANNER_SCAN_BYTES: usize = 2048;
const BANNERS: &[&str] = &[
    "@generated",
    "Code generated by",
    "This file is automatically generated",
    "AUTO-GENERATED",
];

const FILENAME_PATTERNS: &[&str] = &[
    ".pb.go", ".gen.ts", ".gen.rs", "_pb2.py", ".pb.cc",
];

const EXACT_FILENAMES: &[&str] = &["bindings/index.ts"];

pub struct Detection<'a> {
    pub banner_line: &'a str,
}

pub fn detect<'a>(path: &str, content: &'a str) -> Option<Detection<'a>> {
    let scan_end = content.len().min(BANNER_SCAN_BYTES);
    let head = &content[..scan_end];

    // Case-insensitive "DO NOT EDIT" + case-sensitive others.
    let head_lower = head.to_lowercase();
    for line in head.lines() {
        let l = line.trim();
        if BANNERS.iter().any(|b| l.contains(b))
            || head_lower.contains("do not edit") && l.to_lowercase().contains("do not edit")
        {
            return Some(Detection { banner_line: line });
        }
    }

    // Filename-pattern paths fall back to the first non-blank line as the banner.
    let path_l = path.to_lowercase();
    let pattern_match = FILENAME_PATTERNS.iter().any(|p| path_l.ends_with(p))
        || EXACT_FILENAMES.iter().any(|p| path_l.ends_with(p));
    if pattern_match {
        let first_non_blank = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        return Some(Detection { banner_line: first_non_blank });
    }

    None
}

pub fn mark(path: &str, content: &str, hash_prefix: &str) -> Option<String> {
    let det = detect(path, content)?;
    Some(format!(
        "{}\n[GENERATED FILE — body suppressed | sha: {hash_prefix}]\n",
        det.banner_line.trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_at_generated_banner() {
        let content = "// @generated by protobuf\npackage foo\n";
        let d = detect("foo.go", content).unwrap();
        assert!(d.banner_line.contains("@generated"));
    }

    #[test]
    fn detects_do_not_edit_banner_case_insensitive() {
        let content = "// DO NOT EDIT - generated by tool\n";
        assert!(detect("x.ts", content).is_some());
        let lower = "// do not edit - generated\n";
        assert!(detect("x.ts", lower).is_some());
    }

    #[test]
    fn detects_via_pb_go_suffix() {
        let content = "package proto\n\nfunc x() {}\n";
        let d = detect("api.pb.go", content).unwrap();
        assert_eq!(d.banner_line, "package proto");
    }

    #[test]
    fn detects_via_bindings_path() {
        let content = "export const x = 1;\n";
        assert!(detect("frontend/src/bindings/index.ts", content).is_some());
    }

    #[test]
    fn does_not_detect_handwritten_source() {
        let content = "// Comment about generated tests\nfn main() {}\n";
        assert!(detect("src/main.rs", content).is_none());
    }

    #[test]
    fn mark_replaces_body_with_banner_plus_marker() {
        let content = "// @generated by foo\nfunc x() {}\nfunc y() {}\n";
        let out = mark("a.pb.go", content, "abc123").unwrap();
        assert!(out.starts_with("// @generated by foo\n"));
        assert!(out.contains("[GENERATED FILE — body suppressed"));
        assert!(out.contains("sha: abc123"));
        assert!(!out.contains("func y"), "body should be suppressed");
    }
}
```

In `mod.rs`:

```rust
pub mod mark_generated;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p projectpacker-core transforms::mark_generated --tests`
Expected: 6/6 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/transforms/mark_generated.rs crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): add generated-file detection (banner + filename)

Banner scan over the first 2KB (@generated, DO NOT EDIT case-insensitive,
Code generated by, etc.) plus filename pattern fallback (*.pb.go,
*.gen.ts, _pb2.py, bindings/index.ts, etc.). Replaces body with the
detected banner line + a SHA-pinned marker.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task C4: Wire semantic transforms into the pipeline

**Files:**
- Modify: `crates/core/src/transforms/mod.rs`

- [ ] **Step 1: Extend run_transform_phase**

In `crates/core/src/transforms/mod.rs`, add the three new semantic blocks to `run_transform_phase` AFTER the existing `if opts.dedup_files { … }` block and BEFORE the closing `let elapsed`:

```rust
    if opts.collapse_lockfiles {
        let _ = tx.send(ProgressEvent::TransformStart { id: "collapse_lockfiles".into() });
        let r = per_file_with_path(entries, "collapse_lockfiles", |path, content, sha| {
            collapse_lockfile::collapse(path, content, sha)
        });
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    if opts.collapse_minified {
        let _ = tx.send(ProgressEvent::TransformStart { id: "collapse_minified".into() });
        let r = per_file_with_path(entries, "collapse_minified", |_path, content, sha| {
            collapse_minified::collapse(content, sha)
        });
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    if opts.mark_generated {
        let _ = tx.send(ProgressEvent::TransformStart { id: "mark_generated".into() });
        let r = per_file_with_path(entries, "mark_generated", |path, content, sha| {
            mark_generated::mark(path, content, sha)
        });
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
```

Add `pub mod collapse_lockfile; pub mod collapse_minified; pub mod mark_generated;` near the top alongside the existing module declarations.

Add the `per_file_with_path` helper after `per_file`:

```rust
/// Like `per_file`, but the transform fn takes (path, content, sha_prefix).
fn per_file_with_path(
    entries: &mut [FileEntry],
    id: &str,
    f: impl Fn(&str, &str, &str) -> Option<String> + Sync,
) -> TransformReport {
    use rayon::prelude::*;
    let start = Instant::now();
    let changes: Vec<(usize, String, u64)> = entries
        .par_iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let sha_prefix: String = e.hash.chars().take(12).collect();
            let before = e.content.len() as u64;
            let new_content = f(&e.path, &e.content, &sha_prefix)?;
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
        id: id.into(), bytes_saved, files_touched,
        elapsed_ms: start.elapsed().as_millis() as u32,
    }
}
```

- [ ] **Step 2: Add an integration test**

In the `tests` mod inside `crates/core/src/transforms/mod.rs`:

```rust
#[test]
fn semantic_transforms_engage_when_toggled_on() {
    use std::sync::mpsc;
    let big_lockfile = (0..100).map(|i| format!("  \"d{i}\": \"1.0\"")).collect::<Vec<_>>().join("\n");
    let minified: String = "a=1;".repeat(1000);
    let generated = "// @generated by build\nfunc x() {}\n".to_string();
    let mut entries = vec![
        FileEntry { path: "package-lock.json".into(), content: big_lockfile.clone(),
                    bytes: big_lockfile.len() as u64, tokens: None, hash: "h1h1h1h1h1h1h1h1".into() },
        FileEntry { path: "bundle.min.js".into(), content: minified.clone(),
                    bytes: minified.len() as u64, tokens: None, hash: "h2h2h2h2h2h2h2h2".into() },
        FileEntry { path: "api.pb.go".into(), content: generated.clone(),
                    bytes: generated.len() as u64, tokens: None, hash: "h3h3h3h3h3h3h3h3".into() },
    ];
    let opts = PackOptions {
        dedup_files: false, trim_trailing_ws: false,
        collapse_blank_lines: false, normalize_line_endings: false,
        collapse_lockfiles: true, collapse_minified: true, mark_generated: true,
        ..PackOptions::default()
    };
    let (tx, _rx) = mpsc::channel();
    let (reports, _) = run_transform_phase(&mut entries, &opts, &tx);
    let ids: Vec<&str> = reports.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["collapse_lockfiles", "collapse_minified", "mark_generated"]);
    assert!(entries[0].content.contains("[COMPRESSED: lockfile"));
    assert!(entries[1].content.contains("[MINIFIED BUNDLE:"));
    assert!(entries[2].content.contains("[GENERATED FILE"));
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p projectpacker-core transforms --tests`
Expected: all tests pass including the new integration test.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): wire collapse_lockfiles + collapse_minified + mark_generated

Three new opt-in semantic transforms run after the lossless 4. Each uses
the existing BLAKE3 hash for the SHA-pinned marker. Parallelized via the
new per_file_with_path helper.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase D — Code shaping 3

### Task D1: Move existing compress + remove_comments into transforms module

**Files:**
- Create: `crates/core/src/transforms/compress_skeleton.rs`
- Create: `crates/core/src/transforms/strip_comments.rs`
- Modify: `crates/core/src/pack/orchestrator.rs` — remove existing inline calls
- Modify: `crates/core/src/transforms/mod.rs` — wire new modules

- [ ] **Step 1: Create the wrapper modules**

`crates/core/src/transforms/compress_skeleton.rs`:

```rust
//! Thin wrapper around tree_sitter_compress::compress, gated by the
//! `compress` option. Returns Some(new) only when a language was detected
//! AND the content actually changed.

use crate::tree_sitter_compress;

pub fn apply(path: &str, content: &str) -> Option<String> {
    let lang = tree_sitter_compress::detect_language(path)?;
    let new = tree_sitter_compress::compress(content, lang);
    if new == content { None } else { Some(new) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_unknown_language() {
        assert!(apply("README.md", "# title\n").is_none());
    }

    #[test]
    fn compresses_rust_file() {
        let src = "fn add(a: i32, b: i32) -> i32 { a + b }\n";
        let out = apply("a.rs", src).expect("should compress");
        assert!(out.contains("fn add"));
        assert!(!out.contains("a + b"));
    }
}
```

`crates/core/src/transforms/strip_comments.rs`:

```rust
//! Thin wrapper around tree_sitter_compress::remove_comments.

use crate::tree_sitter_compress;

pub fn apply(path: &str, content: &str) -> Option<String> {
    let lang = tree_sitter_compress::detect_language(path)?;
    let new = tree_sitter_compress::remove_comments(content, lang);
    if new == content { None } else { Some(new) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_unknown_language() {
        assert!(apply("data.csv", "a,b,c\n").is_none());
    }

    #[test]
    fn strips_rust_comments() {
        let src = "// hello\nfn x() {}\n";
        let out = apply("a.rs", src).expect("should strip");
        assert!(!out.contains("hello"));
        assert!(out.contains("fn x"));
    }
}
```

- [ ] **Step 2: Wire into mod.rs and remove orchestrator inline calls**

In `crates/core/src/transforms/mod.rs`, add:

```rust
pub mod compress_skeleton;
pub mod strip_comments;
```

In `run_transform_phase` AFTER the `mark_generated` block, add:

```rust
    if opts.remove_comments {
        let _ = tx.send(ProgressEvent::TransformStart { id: "remove_comments".into() });
        let r = per_file_with_path(entries, "remove_comments", |path, content, _sha| {
            strip_comments::apply(path, content)
        });
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
    if opts.compress {
        let _ = tx.send(ProgressEvent::TransformStart { id: "compress".into() });
        let r = per_file_with_path(entries, "compress", |path, content, _sha| {
            compress_skeleton::apply(path, content)
        });
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
```

In `crates/core/src/pack/orchestrator.rs`, find `run_process_phase`. Locate the block:

```rust
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
```

Replace with:

```rust
            // Comment-stripping and skeleton-compress are now handled by the
            // transform phase (see crates/core/src/transforms/{strip_comments,compress_skeleton}.rs).
            let content: String = raw;
```

This is a behavior-preserving move because the transform phase now invokes both with identical `path` and `content` inputs.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --workspace --tests`
Expected: all 250+ tests still pass (the old orchestrator tests that exercised compress/remove_comments still pass because the transforms now run in the new phase with the same option flags).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/transforms/ crates/core/src/pack/orchestrator.rs
git commit -m "$(cat <<'EOF'
refactor(transforms): move compress + remove_comments into transform phase

Behavior-preserving. The old inline calls in run_process_phase are
replaced by thin wrappers in transforms/{compress_skeleton,strip_comments}.rs
that are gated by the existing opts.compress / opts.remove_comments flags.
Now both transforms produce TransformReports like the others.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task D2: Implement elide_types.rs

**Files:**
- Create: `crates/core/src/transforms/elide_types.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/transforms/elide_types.rs`:

```rust
//! TypeScript-only: strip `export type { ... } from "..."` re-export lines.
//! Uses the existing tree-sitter typescript grammar via tree_sitter_compress::PooledParser.
//!
//! Scope: only matches `export type { Name1, Name2 } from "module"` — i.e.
//! type-only RE-exports. Leaves `export type Foo = ...` declarations alone.

use crate::tree_sitter_compress;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};
use std::sync::OnceLock;

fn lang() -> tree_sitter::Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn ts_query() -> &'static Query {
    static Q: OnceLock<Query> = OnceLock::new();
    Q.get_or_init(|| {
        // Match `export type { ... } from "..."` re-export statements.
        let src = r#"(export_statement
            (export_clause) @clause
            (string) @source) @stmt"#;
        Query::new(&lang(), src).expect("type-elision query must compile")
    })
}

pub fn apply(path: &str, content: &str) -> Option<String> {
    if !(path.ends_with(".ts") || path.ends_with(".tsx")) { return None; }
    let mut parser = Parser::new();
    if parser.set_language(&lang()).is_err() { return None; }
    let tree = parser.parse(content, None)?;
    let bytes = content.as_bytes();
    let mut cursor = QueryCursor::new();
    // Collect byte-range of each matching export_statement whose source begins
    // with `export type {` (i.e. is a type-only re-export).
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut iter = cursor.matches(ts_query(), tree.root_node(), bytes);
    while let Some(m) = iter.next() {
        // The whole-statement capture is the last (@stmt). We need to inspect
        // the prefix to confirm it's `export type` and not plain `export`.
        if let Some(stmt) = m.captures.iter().find(|c| c.index == 2 /* @stmt index */ ) {
            let start = stmt.node.start_byte();
            let end = stmt.node.end_byte();
            let snippet = &content[start..end];
            if snippet.trim_start().starts_with("export type ") {
                // Include the trailing newline if present.
                let end_with_nl = if end < bytes.len() && bytes[end] == b'\n' { end + 1 } else { end };
                ranges.push((start, end_with_nl));
            }
        }
    }
    if ranges.is_empty() { return None; }
    ranges.sort_by_key(|r| r.0);
    let mut out = String::with_capacity(content.len());
    let mut pos = 0usize;
    for (s, e) in &ranges {
        if *s > pos { out.push_str(&content[pos..*s]); }
        pos = *e;
    }
    if pos < content.len() { out.push_str(&content[pos..]); }
    let _ = tree_sitter_compress::detect_language(path); // keep the module link
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_type_only_reexport_line() {
        let src = "export type { Foo, Bar } from \"./types\";\nconst x = 1;\n";
        let out = apply("a.ts", src).expect("should change");
        assert!(!out.contains("export type {"));
        assert!(out.contains("const x = 1;"));
    }

    #[test]
    fn leaves_type_alias_declarations_alone() {
        let src = "export type Foo = string;\nconst x = 1;\n";
        assert!(apply("a.ts", src).is_none());
    }

    #[test]
    fn leaves_value_reexports_alone() {
        let src = "export { Foo } from \"./types\";\nconst x = 1;\n";
        assert!(apply("a.ts", src).is_none());
    }

    #[test]
    fn skips_non_typescript_files() {
        let src = "export type { Foo } from \"./x\";\n";
        assert!(apply("a.js", src).is_none());
    }
}
```

- [ ] **Step 2: Wire into mod.rs**

In `crates/core/src/transforms/mod.rs`, add `pub mod elide_types;` and append to `run_transform_phase` AFTER the `compress` block:

```rust
    if opts.elide_type_only_exports {
        let _ = tx.send(ProgressEvent::TransformStart { id: "elide_type_only_exports".into() });
        let r = per_file_with_path(entries, "elide_type_only_exports", |path, content, _sha| {
            elide_types::apply(path, content)
        });
        let _ = tx.send(ProgressEvent::TransformDone {
            id: r.id.clone(), bytes_saved: r.bytes_saved, files_touched: r.files_touched,
        });
        reports.push(r);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p projectpacker-core transforms::elide_types --tests`
Expected: 4/4 PASS.

If the tree-sitter query `@stmt` index check (`c.index == 2`) is wrong (depends on capture order in the query AST), debug by printing capture names. The robust alternative is to look up by name:

```rust
let stmt_idx = ts_query().capture_index_for_name("stmt").unwrap();
if let Some(stmt) = m.captures.iter().find(|c| c.index == stmt_idx) { ... }
```

If you needed that fix, update the code accordingly before committing.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/transforms/elide_types.rs crates/core/src/transforms/mod.rs
git commit -m "$(cat <<'EOF'
feat(transforms): add elide_type_only_exports for TypeScript

Strips 'export type { ... } from "..."' re-export lines via tree-sitter.
Leaves type-alias declarations and value re-exports untouched.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase E — Emit changes

### Task E1: XML emitter — duplicate-of attr + compression_report block

**Files:**
- Modify: `crates/core/src/pack/xml.rs`

- [ ] **Step 1: Read xml.rs to locate the existing patterns**

Read `crates/core/src/pack/xml.rs` to find the `documents()` builder method and the `security_report_block()` builder method.

- [ ] **Step 2: Add `duplicate-of` attribute support**

In `documents()` (the cxml schema emitter), detect content matching the dedup marker and emit a self-closing `<document>` with `duplicate-of` + `sha` attrs instead of the full `<document>...</document>`.

Detection in code (pseudocode — adapt to the actual builder API):

```rust
// Inside the documents() loop:
const DUP_PREFIX: &str = "[DUPLICATE OF: ";
if entry.content.starts_with(DUP_PREFIX) {
    // Extract "[DUPLICATE OF: <path> | sha: <prefix>]"
    let rest = &entry.content[DUP_PREFIX.len()..];
    if let Some((path_part, after)) = rest.split_once(" | sha: ") {
        if let Some(sha_part) = after.split(']').next() {
            // Emit: <document index="N" source="entry.path" duplicate-of="path_part" sha="sha_part" />
            self.write_self_closing_document(index, &entry.path, path_part, sha_part);
            continue;
        }
    }
}
// Otherwise: existing full-body emit path.
```

Implement `write_self_closing_document` similar to existing document writing but as a self-closing tag with the two new attributes (use `quick-xml` escape helpers as elsewhere in this file).

- [ ] **Step 3: Add the `compression_report` block**

After `security_report_block(&all_redactions)` is called in `orchestrator::run_emit_phase` (which calls `builder.security_report_block(...)` in `xml.rs`), add a sibling `.compression_report_block(stats)` chained call.

In `xml.rs`, add the method:

```rust
pub fn compression_report_block(&mut self, stats: &PackStats) -> &mut Self {
    if stats.transforms.is_empty() { return self; }
    self.indent().push_str("<compression_report>\n");
    for r in &stats.transforms {
        self.indent_inner().push_str(&format!(
            "<transform id=\"{}\" bytes_saved=\"{}\" files_touched=\"{}\" elapsed_ms=\"{}\"/>\n",
            quick_xml::escape::escape(&r.id),
            r.bytes_saved, r.files_touched, r.elapsed_ms,
        ));
    }
    self.indent().push_str("</compression_report>\n");
    self
}
```

(Pattern-match the existing `security_report_block` implementation for indent helpers; the call signatures above are illustrative.)

In `crates/core/src/pack/orchestrator.rs::run_emit_phase`, update the XML branch:

```rust
            builder
                .open_repository()
                .raw_block(&protocol_block)
                .stats_block(label, opts, stats, entries, all_redactions)
                .security_report_block(all_redactions)
                .compression_report_block(stats)        // NEW
                .directory_structure(&dir_paths);
```

- [ ] **Step 4: Write the integration test**

Add to `crates/core/src/pack/orchestrator.rs::tests`:

```rust
#[test]
fn pack_emits_compression_report_block_when_transforms_applied() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("a.txt"), "trailing   \n").unwrap();
    let opts = PackOptions {
        goal: "x".into(),
        secret_scan: false,
        count_tokens: false,
        ..PackOptions::default() // 4 lossless ON by default
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack(
        &PackTarget::Folder(d.path().to_path_buf()),
        &opts, tx, "job-cr", CancellationToken::new(), None,
    ).unwrap();
    assert!(result.output.contains("<compression_report>"));
    assert!(result.output.contains("trim_trailing_ws"));
    assert!(!result.stats.transforms.is_empty());
}

#[test]
fn pack_emits_duplicate_of_attr_for_deduped_files() {
    let d = tempdir().unwrap();
    let body = "Apache 2.0 license body\n";
    fs::write(d.path().join("LICENSE"), body).unwrap();
    let vendor = d.path().join("vendor").join("foo");
    fs::create_dir_all(&vendor).unwrap();
    fs::write(vendor.join("LICENSE"), body).unwrap();
    let opts = PackOptions {
        goal: "x".into(),
        secret_scan: false,
        count_tokens: false,
        respect_gitignore: false,
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack(
        &PackTarget::Folder(d.path().to_path_buf()),
        &opts, tx, "job-dup", CancellationToken::new(), None,
    ).unwrap();
    // The second copy must reference the first via duplicate-of attribute.
    assert!(result.output.contains("duplicate-of=\"LICENSE\""),
        "output should contain duplicate-of attr; got:\n{}", result.output);
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p projectpacker-core pack_emits --tests`
Expected: 2 new tests PASS.
Run: `cargo test --workspace --tests`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/pack/xml.rs crates/core/src/pack/orchestrator.rs
git commit -m "$(cat <<'EOF'
feat(pack): emit <compression_report> + duplicate-of attribute in XML

Self-closing <document path=... duplicate-of=... sha=... /> for deduped
files (no body). New <compression_report> block lists every applied
transform with bytes_saved + files_touched + elapsed_ms.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task E2: Markdown + Plain emitter — compression report section

**Files:**
- Modify: `crates/core/src/pack/markdown.rs`
- Modify: `crates/core/src/pack/plain.rs`

- [ ] **Step 1: Update markdown.rs**

Locate the existing security-report section emit in `markdown::render`. Right after it, append a compression report:

```rust
// After security_report section emit:
if !stats.transforms.is_empty() {
    out.push_str("\n## Compression report\n\n");
    out.push_str("| Transform | Bytes saved | Files touched | Elapsed (ms) |\n");
    out.push_str("|---|---:|---:|---:|\n");
    for r in &stats.transforms {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.id, r.bytes_saved, r.files_touched, r.elapsed_ms,
        ));
    }
}
```

(Body markers — `[DUPLICATE OF: …]` etc. — already pass through to the MD body unchanged. No additional MD-specific work needed for the markers themselves.)

- [ ] **Step 2: Update plain.rs**

Locate the existing security-report section in `plain::render`. Append:

```rust
if !stats.transforms.is_empty() {
    out.push_str("\n=== Compression report ===\n");
    for r in &stats.transforms {
        out.push_str(&format!(
            "  {:30}  bytes_saved={:>10}  files_touched={:>4}  elapsed_ms={:>4}\n",
            r.id, r.bytes_saved, r.files_touched, r.elapsed_ms,
        ));
    }
}
```

- [ ] **Step 3: Test both formats**

Add to `pack_integration.rs`:

```rust
#[test]
fn markdown_emit_includes_compression_report() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("a.txt"), "trailing  \n").unwrap();
    let opts = PackOptions {
        goal: "x".into(),
        format: PackFormat::Markdown,
        secret_scan: false,
        count_tokens: false,
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack(
        &PackTarget::Folder(d.path().to_path_buf()),
        &opts, tx, "job-md-cr", CancellationToken::new(), None,
    ).unwrap();
    assert!(result.output.contains("## Compression report"));
    assert!(result.output.contains("trim_trailing_ws"));
}

#[test]
fn plain_emit_includes_compression_report() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("a.txt"), "trailing  \n").unwrap();
    let opts = PackOptions {
        goal: "x".into(),
        format: PackFormat::PlainText,
        secret_scan: false,
        count_tokens: false,
        ..PackOptions::default()
    };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack(
        &PackTarget::Folder(d.path().to_path_buf()),
        &opts, tx, "job-plain-cr", CancellationToken::new(), None,
    ).unwrap();
    assert!(result.output.contains("=== Compression report ==="));
    assert!(result.output.contains("trim_trailing_ws"));
}
```

(Add the right `use` statements at the top of the test file: `tempfile::tempdir`, `std::fs`, etc., matching the existing ones.)

- [ ] **Step 4: Run tests**

Run: `cargo test --test pack_integration --workspace`
Expected: all integration tests pass including 2 new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/pack/markdown.rs crates/core/src/pack/plain.rs crates/core/tests/pack_integration.rs
git commit -m "$(cat <<'EOF'
feat(pack): emit compression report in Markdown + Plain formats

Markdown gets a "## Compression report" table; Plain gets a "=== Compression
report ===" delimited block. Body markers (DUPLICATE OF, COMPRESSED, etc.)
already pass through both formats unchanged.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task E3: Snapshot test for compression_report shape

**Files:**
- Modify: `crates/core/tests/protocol_golden.rs`

- [ ] **Step 1: Add a snapshot test**

Append to `crates/core/tests/protocol_golden.rs`:

```rust
#[test]
fn compression_report_block_shape() {
    use projectpacker_core::types::{PackStats, TransformReport, TokensPerModel};
    let stats = PackStats {
        files_total: 1,
        files_included: 1,
        files_skipped: 0,
        bytes_total: 100,
        tokens_total: None,
        tokens_per_model: None,
        secrets_found: 0,
        duration_ms: 10,
        walk_ms: 1, process_ms: 1, secret_scan_ms: None, tokenize_ms: None,
        emit_ms: 1,
        transform_phase_ms: 2,
        transforms: vec![
            TransformReport { id: "trim_trailing_ws".into(), bytes_saved: 12, files_touched: 1, elapsed_ms: 0 },
            TransformReport { id: "dedup_files".into(), bytes_saved: 4096, files_touched: 2, elapsed_ms: 1 },
        ],
    };
    // Render just the report block. The simplest path: construct a builder,
    // call compression_report_block, finish, snapshot.
    use projectpacker_core::pack::xml::XmlBuilder;
    let mut b = XmlBuilder::with_capacity(128);
    b.compression_report_block(&stats);
    insta::assert_snapshot!(b.finish());
}
```

- [ ] **Step 2: Run and accept snapshot**

Run: `cargo test --test protocol_golden compression_report_block_shape -- --nocapture`
Expected: FAIL — snapshot doesn't exist.

Review the proposed snapshot and accept:

```
cargo insta accept
```

Run: `cargo test --test protocol_golden compression_report_block_shape`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/tests/protocol_golden.rs crates/core/tests/snapshots/
git commit -m "$(cat <<'EOF'
test(protocol): snapshot the <compression_report> block shape

Guards against accidental format drift in the per-transform reporting
markup that downstream AI consumers parse.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase F — UI

### Task F1: Extend Zustand store with new toggles

**Files:**
- Modify: `frontend/src/lib/store.ts`

- [ ] **Step 1: Locate the current PackOptions slice**

Read `frontend/src/lib/store.ts`. Find where `compress` and `remove_comments` are persisted/exposed.

- [ ] **Step 2: Add the 8 new boolean fields**

In the store's options shape (and the persistence schema if there's a versioned migration step), add:

```ts
dedup_files: boolean;
trim_trailing_ws: boolean;
collapse_blank_lines: boolean;
normalize_line_endings: boolean;
collapse_lockfiles: boolean;
collapse_minified: boolean;
mark_generated: boolean;
elide_type_only_exports: boolean;
```

Defaults — match the Rust defaults exactly:

```ts
const DEFAULT_TRANSFORM_OPTS = {
    dedup_files: true,
    trim_trailing_ws: true,
    collapse_blank_lines: true,
    normalize_line_endings: true,
    collapse_lockfiles: false,
    collapse_minified: false,
    mark_generated: false,
    elide_type_only_exports: false,
};
```

If there's a `patchOptions` action: nothing to change — it already accepts a partial. If there are individual setter actions for `compress` / `remove_comments`, add the same shape for each new toggle (or — better — generalize to a generic per-option setter to avoid 8 near-identical functions).

- [ ] **Step 3: Typecheck**

Run: `pnpm --dir frontend typecheck`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/lib/store.ts
git commit -m "$(cat <<'EOF'
feat(store): add 8 new compression toggles to Zustand state

Mirrors PackOptions on the Rust side. 4 default-true (lossless),
4 default-false (semantic + lossy).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task F2: Build TransformRow + CompressionPanel components

**Files:**
- Create: `frontend/src/components/pack/TransformRow.tsx`
- Create: `frontend/src/components/pack/CompressionPanel.tsx`

- [ ] **Step 1: Implement TransformRow**

Create `frontend/src/components/pack/TransformRow.tsx`:

```tsx
import { Toggle } from "./Toggle";

export interface TransformRowProps {
  label: string;
  description: string;
  checked: boolean;
  onToggle: (value: boolean) => void;
  /** Bytes saved by this transform on the last pack, or undefined if not run yet. */
  bytesSaved?: number;
  /** True if this transform ran but had zero eligible files. */
  noEligibleFiles?: boolean;
}

function formatBytes(n: number): string {
  if (n === 0) return "0 B";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

export function TransformRow({
  label,
  description,
  checked,
  onToggle,
  bytesSaved,
  noEligibleFiles,
}: TransformRowProps) {
  const savings = bytesSaved === undefined
    ? "—"
    : noEligibleFiles
      ? "n/a — no eligible files"
      : `${formatBytes(bytesSaved)} saved`;
  return (
    <div className="flex items-center justify-between py-1.5 px-2 rounded hover:bg-surface-2/40">
      <div className="flex items-center gap-3 min-w-0">
        <Toggle checked={checked} onChange={onToggle} />
        <div className="min-w-0">
          <div className="text-sm font-medium truncate">{label}</div>
        </div>
        <span
          className="text-xs text-fg-muted cursor-help"
          title={description}
          aria-label={`Description: ${description}`}
        >ⓘ</span>
      </div>
      <div className="text-xs text-transform-savings tabular-nums whitespace-nowrap">
        {savings}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Implement CompressionPanel**

Create `frontend/src/components/pack/CompressionPanel.tsx`:

```tsx
import { useState } from "react";
import { useStore } from "../../lib/store";
import { TransformRow } from "./TransformRow";

interface TransformSpec {
  key:
    | "dedup_files"
    | "trim_trailing_ws"
    | "collapse_blank_lines"
    | "normalize_line_endings"
    | "collapse_lockfiles"
    | "collapse_minified"
    | "mark_generated"
    | "compress"
    | "remove_comments"
    | "elide_type_only_exports";
  /** Matches TransformReport.id on the wire. */
  id: string;
  label: string;
  description: string;
}

const LOSSLESS: TransformSpec[] = [
  { key: "dedup_files", id: "dedup_files",
    label: "Dedup duplicate files",
    description: "Identical files (LICENSE copies, vendored libs) become a content-pointer." },
  { key: "trim_trailing_ws", id: "trim_trailing_ws",
    label: "Trim trailing whitespace",
    description: "Strips trailing spaces and tabs from every line." },
  { key: "collapse_blank_lines", id: "collapse_blank_lines",
    label: "Collapse blank lines",
    description: "Runs of 3+ blank lines collapse to 2." },
  { key: "normalize_line_endings", id: "normalize_line_endings",
    label: "Normalize line endings (CRLF → LF)",
    description: "CRLF and lone CR are converted to LF." },
];

const SEMANTIC: TransformSpec[] = [
  { key: "collapse_lockfiles", id: "collapse_lockfiles",
    label: "Collapse lockfiles",
    description: "package-lock.json, pnpm-lock.yaml, Cargo.lock, etc.: keep head/tail + marker." },
  { key: "collapse_minified", id: "collapse_minified",
    label: "Collapse minified bundles",
    description: "Single-line or extreme-variance bundles: replace body with a marker." },
  { key: "mark_generated", id: "mark_generated",
    label: "Mark generated files",
    description: "Files with @generated banners or *.pb.go/.gen.ts patterns: suppress body." },
];

const LOSSY: TransformSpec[] = [
  { key: "compress", id: "compress",
    label: "Skeleton-compress functions",
    description: "Replace function/class/method bodies with a skeleton (rs/py/js/ts)." },
  { key: "remove_comments", id: "remove_comments",
    label: "Strip comments",
    description: "Remove comments from rs/py/js/ts files." },
  { key: "elide_type_only_exports", id: "elide_type_only_exports",
    label: "Elide TypeScript type-only re-exports",
    description: "Strip 'export type { … } from …' lines." },
];

function totalBytes(transforms: { id: string; bytes_saved: number }[]) {
  return transforms.reduce((acc, t) => acc + t.bytes_saved, 0);
}

function formatBytes(n: number): string {
  if (n === 0) return "0 B";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

export function CompressionPanel() {
  const [open, setOpen] = useState(false);
  const options = useStore(s => s.options);
  const patch = useStore(s => s.patchOptions);
  const transforms = useStore(s => s.lastStats?.transforms ?? []);

  const reportFor = (id: string) => transforms.find(t => t.id === id);

  const enabledCount = [
    ...LOSSLESS, ...SEMANTIC, ...LOSSY,
  ].filter(spec => options[spec.key]).length;
  const totalSaved = totalBytes(transforms);

  function renderGroup(title: string, caption: string, specs: TransformSpec[]) {
    const allOn = specs.every(s => options[s.key]);
    const allOff = specs.every(s => !options[s.key]);
    const someOn = specs.filter(s => options[s.key]).length;
    const chipLabel = allOn ? "✓ all on" : allOff ? "✗ all off" : `~ ${someOn} of ${specs.length} on`;
    const toggleAll = () => {
      const next = !allOn;
      const patch_: Record<string, boolean> = {};
      for (const s of specs) patch_[s.key] = next;
      patch(patch_);
    };
    return (
      <div key={title} className="mt-3">
        <div className="flex items-center justify-between px-2 mb-1">
          <div>
            <span className="text-[13px] font-semibold tracking-wide">{title}</span>
            <span className="text-[11px] text-fg-muted ml-2">{caption}</span>
          </div>
          <button
            type="button"
            className="text-[11px] text-fg-muted hover:text-fg-primary cursor-pointer"
            onClick={toggleAll}
          >{chipLabel}</button>
        </div>
        <div>
          {specs.map(spec => {
            const report = reportFor(spec.id);
            return (
              <TransformRow
                key={spec.key}
                label={spec.label}
                description={spec.description}
                checked={!!options[spec.key]}
                onToggle={v => patch({ [spec.key]: v })}
                bytesSaved={report?.bytes_saved}
                noEligibleFiles={!!report && report.files_touched === 0}
              />
            );
          })}
        </div>
      </div>
    );
  }

  return (
    <div className="border border-border rounded-lg">
      <button
        type="button"
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-surface-2/40"
        onClick={() => setOpen(!open)}
        aria-expanded={open}
      >
        <span className="text-sm font-medium">
          {open ? "▾" : "▸"} Compression
          <span className="text-fg-muted text-xs ml-2">
            {enabledCount} of 10 enabled
          </span>
        </span>
        <span className="text-xs text-transform-savings">
          {transforms.length > 0 ? `Last run: ${formatBytes(totalSaved)} saved` : ""}
        </span>
      </button>
      {open && (
        <div className="px-2 pb-3">
          {renderGroup("LOSSLESS", "applied by default", LOSSLESS)}
          {renderGroup("SEMANTIC", "opt-in, no information loss", SEMANTIC)}
          {renderGroup("CODE SHAPING", "opt-in, modifies code (lossy)", LOSSY)}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Add the theme token**

In `frontend/src/styles/globals.css`, inside the existing Tailwind v4 `@theme` block, add:

```css
--color-transform-savings: oklch(0.65 0.09 145);
```

This drives the `text-transform-savings` class above. If the existing color tokens use a different naming scheme, match that scheme.

- [ ] **Step 4: Typecheck**

Run: `pnpm --dir frontend typecheck`
Expected: 0 errors (or errors only from missing `lastStats` on the store — see next task to address).

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/pack/CompressionPanel.tsx frontend/src/components/pack/TransformRow.tsx frontend/src/styles/globals.css
git commit -m "$(cat <<'EOF'
feat(ui): add CompressionPanel + TransformRow components

Collapsible disclosure with three grouped sections (lossless / semantic /
code shaping). Per-row savings sourced from PackStats.transforms by id.
Group chip bulk-toggles. New --color-transform-savings theme token.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task F3: Wire CompressionPanel into Pack.tsx + handle live updates

**Files:**
- Modify: `frontend/src/lib/store.ts` (add lastStats selector if missing)
- Modify: `frontend/src/lib/use-pack-job.ts` (handle TransformStart/Done events)
- Modify: `frontend/src/routes/Pack.tsx`

- [ ] **Step 1: Ensure store exposes `lastStats`**

In `frontend/src/lib/store.ts`, confirm there's a `lastStats: PackStats | null` slice updated by the pack-job hook on completion. If not present, add it: receiving `Done` or the final `PackResult` writes `state.lastStats = result.stats`.

Also add live-update behavior: on `TransformDone` event, patch `state.lastStats.transforms` in-place — append a `TransformReport`-shaped entry if not present, or update if already there:

```ts
// In the pack-event reducer/handler:
case "TransformDone": {
    const ev = event.payload as { id: string; bytes_saved: number; files_touched: number };
    state.lastStats = state.lastStats ?? emptyStats();
    const idx = state.lastStats.transforms.findIndex(t => t.id === ev.id);
    const entry = { id: ev.id, bytes_saved: ev.bytes_saved, files_touched: ev.files_touched, elapsed_ms: 0 };
    if (idx >= 0) state.lastStats.transforms[idx] = entry;
    else state.lastStats.transforms.push(entry);
    break;
}
```

(Adapt the case-label / dispatch shape to the existing event-handling style — likely `events.ts` or `use-pack-job.ts`.)

- [ ] **Step 2: Mount CompressionPanel in Pack.tsx**

Read `frontend/src/routes/Pack.tsx` and find the existing inline `compress` + `remove_comments` toggles. Remove them. In their place, mount:

```tsx
import { CompressionPanel } from "../components/pack/CompressionPanel";

// ... inside the render ...
<CompressionPanel />
```

If those two toggles were in a "Settings" sub-panel, keep `CompressionPanel` in the same sub-panel position.

- [ ] **Step 3: Typecheck + lint**

Run: `pnpm --dir frontend typecheck`
Expected: 0 errors.
Run: `pnpm --dir frontend lint`
Expected: only pre-existing nits (handoff documents 2 pre-existing formatter nits in `Pack.tsx:245` and `use-drag-drop.ts:39`).

- [ ] **Step 4: Manual smoke test (interactive)**

Run: `pnpm tauri dev`
Manually verify:
1. The Compression disclosure renders.
2. Toggling rows updates the store.
3. The chip bulk-toggles work.
4. After running a pack, per-row savings populate.

If anything looks off, fix inline before committing.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/store.ts frontend/src/lib/use-pack-job.ts frontend/src/routes/Pack.tsx
git commit -m "$(cat <<'EOF'
feat(ui): wire CompressionPanel into Pack screen + live updates

Replaces the inline compress/remove_comments toggles. TransformDone IPC
events update per-row savings live during a pack. lastStats slice
populated on Done.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task F4: Vitest test for CompressionPanel

**Files:**
- Create: `frontend/src/components/pack/CompressionPanel.test.tsx`

- [ ] **Step 1: Write the test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { CompressionPanel } from "./CompressionPanel";

// Mock the store. Adapt the path/shape to the actual store module.
vi.mock("../../lib/store", () => {
  const state = {
    options: {
      dedup_files: true,
      trim_trailing_ws: true,
      collapse_blank_lines: true,
      normalize_line_endings: true,
      collapse_lockfiles: false,
      collapse_minified: false,
      mark_generated: false,
      compress: false,
      remove_comments: false,
      elide_type_only_exports: false,
    },
    lastStats: null,
  };
  const patchOptions = vi.fn((p: Record<string, boolean>) => Object.assign(state.options, p));
  const useStore = <T,>(sel: (s: typeof state & { patchOptions: typeof patchOptions }) => T) =>
    sel({ ...state, patchOptions });
  return { useStore };
});

describe("CompressionPanel", () => {
  it("expands when the header is clicked", () => {
    render(<CompressionPanel />);
    expect(screen.queryByText(/LOSSLESS/)).toBeNull();
    fireEvent.click(screen.getByText(/Compression/));
    expect(screen.getByText(/LOSSLESS/)).toBeTruthy();
  });

  it("shows '4 of 10 enabled' for the conservative default", () => {
    render(<CompressionPanel />);
    expect(screen.getByText(/4 of 10 enabled/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run vitest**

Run: `pnpm --dir frontend test`
Expected: PASS (this becomes the first vitest test in the codebase).

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/pack/CompressionPanel.test.tsx
git commit -m "$(cat <<'EOF'
test(ui): add first Vitest test for CompressionPanel

Covers expand-on-click and conservative-default enabled count. Mocks the
Zustand store with a vi.mock factory.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

# Phase G — Protocol docs + CHANGELOG

### Task G1: Update protocol doc with marker reference

**Files:**
- Modify: `docs/protocol/grok-to-cc-v1.md`

- [ ] **Step 1: Append the markers section**

At the end of `docs/protocol/grok-to-cc-v1.md`, append:

```markdown
## Compression markers

The pack may contain placeholders inserted by lossless compression transforms:

### File-body markers
- `[DUPLICATE OF: <path> | sha: <12-char-prefix>]`
  File is byte-identical to <path>. Consult the named file for content.
- `[COMPRESSED: <reason> | original-bytes: N | sha: <12-char-prefix>]`
  Body was collapsed. <reason> ∈ {lockfile, minified, generated}.
  Lockfile/minified bodies retain first/last N lines; generated bodies retain
  the detection banner.

### XML attribute
- `<document path="..." duplicate-of="..." sha="..." />` — same semantic as
  the body marker; used when the body is empty.

### Compression report
Every pack with at least one transform applied emits `<compression_report>`
(or equivalent Markdown / Plain block) listing every applied transform with
bytes saved, files touched, and elapsed time.

### Executor guidance
Do not treat compression markers as missing content. The original content is
either available in the duplicate's first occurrence or, if compressed, was
deemed low-signal by the user.
```

- [ ] **Step 2: Verify the golden snapshot tests still pass**

The protocol template is embedded in the pack output; snapshot tests in `protocol_golden.rs` may need updating.

Run: `cargo test --test protocol_golden`
- If FAIL: review changes, `cargo insta accept` if intentional, re-run.
- If PASS: nothing more.

- [ ] **Step 3: Commit**

```bash
git add docs/protocol/grok-to-cc-v1.md crates/core/tests/snapshots/
git commit -m "$(cat <<'EOF'
docs(protocol): document compression markers in grok-to-cc-v1

No version bump — markers are self-describing English. Adds reference for
[DUPLICATE OF: …], [COMPRESSED: <reason> …], <document duplicate-of=…/>,
the <compression_report> block, and executor guidance.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task G2: CHANGELOG entry

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add an Unreleased entry**

Under the `## [Unreleased]` heading in `CHANGELOG.md`, append:

```markdown
### Added
- **Compression pipeline (v0.6)** — 10 individually-toggleable content transforms with per-transform savings telemetry.
  - 4 lossless (default ON): `dedup_files`, `trim_trailing_ws`, `collapse_blank_lines`, `normalize_line_endings`.
  - 3 semantic (default OFF): `collapse_lockfiles`, `collapse_minified`, `mark_generated`.
  - 3 code-shaping / lossy (default OFF): `compress` (skeleton, renamed in UI to "Skeleton-compress functions"), `remove_comments`, `elide_type_only_exports` (new).
- `CompressionPanel` UI — collapsible disclosure on the Pack screen with grouped sections and per-row savings.
- `<compression_report>` block in XML output; equivalent table/divider in Markdown and Plain outputs.
- New `ProgressEvent::TransformStart` / `TransformDone` events for live UI updates.
- First Vitest test in the frontend (`CompressionPanel.test.tsx`).

### Changed
- `PackOptions` gains 8 new fields. Old presets deserialize cleanly thanks to `#[serde(default)]`.
- `compress` and `remove_comments` continue to use those exact field names on disk for preset compatibility; UI labels them as "Skeleton-compress functions" and "Strip comments" respectively.

### Internal
- New `crates/core/src/transforms/` module — one file per transform.
- New pack pipeline phase between `process` and `pin-reorder` (`run_transform_phase`).
- `PackStats` gains `transforms: Vec<TransformReport>` and `transform_phase_ms: u32`.
- New `WarningKind::TransformFailed` for per-file per-transform failures.
```

- [ ] **Step 2: Run the verification gate one more time**

Run all in sequence:

```bash
cargo test --workspace --tests
cargo clippy --workspace --all-targets -- -D warnings
pnpm bindings
pnpm --dir frontend typecheck
pnpm --dir frontend lint
pnpm --dir frontend test
```

Expected: every step exits clean (or with only the pre-existing nits documented in `handoff.md`).

- [ ] **Step 3: Final build sanity check**

Run: `pnpm tauri build --no-bundle`
Expected: clean build, `target/release/projectpacker-app.exe` produced.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
docs(changelog): record v0.6 compression pipeline work

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Final verification before push

```bash
cargo test --workspace --tests       # → 250+ passing, 0 failing
cargo clippy --workspace --all-targets -- -D warnings
pnpm bindings
pnpm --dir frontend typecheck
pnpm --dir frontend lint
pnpm --dir frontend test
pnpm tauri build --no-bundle         # 47 MB-ish output
git log --oneline main..HEAD         # review the commit series
git push origin main                  # only if user explicitly authorized
```
