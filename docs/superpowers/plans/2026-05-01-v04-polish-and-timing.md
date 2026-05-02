# v0.4 Polish Implementation Plan — Drag-and-Drop + Per-Phase Timing

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two focused polish improvements for v0.4 — full-window drag-and-drop folder selection, and per-phase timing in `PackStats` surfaced as an inline breakdown row in the result panel.

**Architecture:** Backend wraps the 5 natural orchestrator phases (walk / process / secret_scan / tokenize / emit) with `Instant::now()` and stores per-phase elapsed milliseconds in 5 new `PackStats` fields (3 `u32`, 2 `Option<u32>` for the optional phases). Frontend adds a `useDragDrop` hook that wires Tauri 2's webview-level `onDragDropEvent` to React state, with a full-screen overlay during dragover and a `stat()`-based file-vs-folder check on drop. The result panel gains a `PhaseBreakdown` component below `StatsBar`.

**Tech Stack:** Rust (`std::time::Instant`), Tauri 2 (`getCurrentWebview().onDragDropEvent`), `@tauri-apps/plugin-fs` (`stat`), React 19 + TypeScript 5, Tailwind v4, Zustand, `tauri-specta` for binding regeneration.

**Spec:** `docs/superpowers/specs/2026-05-01-v04-polish-and-timing-design.md`

---

## File Map

### Rust — modify

- `crates/core/src/types.rs` — add 5 fields to `PackStats`; update one test fixture at line 238
- `crates/core/src/pack/orchestrator.rs` — wrap 5 phases with `Instant`; populate new fields in stats constructor at line 354
- `crates/core/src/pack/xml.rs` — update 2 test fixtures (lines 287, 332)
- `crates/core/src/pack/stats.rs` — update `make_stats()` test helper at line 120
- `crates/core/src/pack/plain.rs` — update `stats()` test helper at line 93
- `crates/core/src/pack/markdown.rs` — update `stats()` test helper at line 110
- `crates/app/src/jobs.rs` — update fallback `PackStats` at line 92

### Bindings — auto-regenerated

- `frontend/src/bindings/index.ts` — `cargo run -p projectpacker-app --bin emit-bindings`

### Frontend — create

- `frontend/src/lib/use-drag-drop.ts` — new `useDragDrop` hook

### Frontend — modify

- `frontend/src/routes/Pack.tsx` — wire `useDragDrop`, add `DropOverlay` + `PhaseBreakdown` components

### Docs

- `CHANGELOG.md` — append v0.4 entries

---

## Task 1: Add 5 per-phase fields to `PackStats`

**Files:**
- Modify: `crates/core/src/types.rs`

- [ ] **Step 1: Add the new fields to `PackStats`**

In `crates/core/src/types.rs`, find the `PackStats` struct (around line 75). Add 5 new fields immediately after `duration_ms`. The struct now reads:

```rust
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
```

- [ ] **Step 2: Update the existing `progress_event_done_serializes_with_stats` test**

In the same file, find the test around line 236. Add the 5 new fields to the `PackStats { ... }` literal. The full test reads:

```rust
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
```

- [ ] **Step 3: Run the types tests to verify struct compiles + test passes**

```
cargo test -p projectpacker-core --lib types
```

Expected: 4 passed. The new fields compile and the test populates them.

- [ ] **Step 4: Commit**

```
git add crates/core/src/types.rs
git commit -m "feat(core/types): add per-phase ms fields to PackStats"
```

---

## Task 2: Update all `PackStats` literal sites to include the new fields

**Files:**
- Modify: `crates/core/src/pack/xml.rs` (2 sites)
- Modify: `crates/core/src/pack/stats.rs` (1 site)
- Modify: `crates/core/src/pack/plain.rs` (1 site)
- Modify: `crates/core/src/pack/markdown.rs` (1 site)
- Modify: `crates/app/src/jobs.rs` (1 site)

> **Why this task exists:** Adding fields to a struct without `#[non_exhaustive]` is a compile-break for any literal constructor. Every `PackStats { ... }` in the workspace must add the 5 new fields. The orchestrator's production constructor is updated separately in Task 3 (it gets real `Instant` timing).

- [ ] **Step 1: Update `crates/core/src/pack/xml.rs` line 287 — populated test fixture**

In the test that constructs a `PackStats` with non-zero values, add 5 fields after `duration_ms: 42,`. The full literal reads:

```rust
        let stats = PackStats {
            files_total: 2,
            files_included: 1,
            files_skipped: 1,
            bytes_total: 500,
            tokens_total: Some(100),
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 42,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
        };
```

- [ ] **Step 2: Update `crates/core/src/pack/xml.rs` line 332 — empty/zero test fixture**

The same shape, all-zero/None:

```rust
        let stats = PackStats {
            files_total: 0,
            files_included: 0,
            files_skipped: 0,
            bytes_total: 0,
            tokens_total: None,
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 0,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
        };
```

- [ ] **Step 3: Update `crates/core/src/pack/stats.rs` — `make_stats()` helper at line 119**

```rust
    fn make_stats() -> PackStats {
        PackStats {
            files_total: 0,
            files_included: 0,
            files_skipped: 0,
            bytes_total: 0,
            tokens_total: None,
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 0,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
        }
    }
```

- [ ] **Step 4: Update `crates/core/src/pack/plain.rs` — `stats()` helper at line 92**

```rust
    fn stats() -> PackStats {
        PackStats {
            files_total: 1,
            files_included: 1,
            files_skipped: 0,
            bytes_total: 50,
            tokens_total: Some(20),
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 5,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
        }
    }
```

- [ ] **Step 5: Update `crates/core/src/pack/markdown.rs` — `stats()` helper at line 109**

```rust
    fn stats() -> PackStats {
        PackStats {
            files_total: 2,
            files_included: 2,
            files_skipped: 0,
            bytes_total: 100,
            tokens_total: Some(50),
            tokens_per_model: None,
            secrets_found: 0,
            duration_ms: 10,
            walk_ms: 0,
            process_ms: 0,
            secret_scan_ms: None,
            tokenize_ms: None,
            emit_ms: 0,
        }
    }
```

- [ ] **Step 6: Update `crates/app/src/jobs.rs` — fallback `PackStats` at line 92**

```rust
            stats: projectpacker_core::types::PackStats {
                files_total: 0,
                files_included: 0,
                files_skipped: 0,
                bytes_total: 0,
                tokens_total: None,
                tokens_per_model: None,
                secrets_found: 0,
                duration_ms: 0,
                walk_ms: 0,
                process_ms: 0,
                secret_scan_ms: None,
                tokenize_ms: None,
                emit_ms: 0,
            },
```

- [ ] **Step 7: Verify the workspace still compiles (orchestrator.rs is the only remaining constructor and is updated in Task 3, but it should still compile via `Default`-fallback warnings — actually, no. We must update orchestrator.rs in this task too so the workspace compiles between commits.)**

Update `crates/core/src/pack/orchestrator.rs` line 354. For now, set the 5 new fields to placeholder values so the workspace compiles. The real `Instant`-based values land in Task 3:

```rust
    let stats = PackStats {
        files_total: (outcome.included.len() + outcome.skipped.len()) as u32,
        files_included: entries.len() as u32,
        files_skipped: outcome.skipped.len() as u32,
        bytes_total,
        tokens_total: opts.count_tokens.then_some(tokens_total),
        tokens_per_model,
        secrets_found,
        duration_ms: start.elapsed().as_millis() as u32,
        walk_ms: 0,
        process_ms: 0,
        secret_scan_ms: None,
        tokenize_ms: None,
        emit_ms: 0,
    };
```

- [ ] **Step 8: Run the workspace test suite — every existing test must pass with placeholder zeros**

```
cargo test --workspace --tests
```

Expected: all tests pass. Per-phase fields read as zeros/None for now; orchestrator wires real timing in Task 3.

- [ ] **Step 9: Commit**

```
git add crates/core/src/pack/xml.rs crates/core/src/pack/stats.rs crates/core/src/pack/plain.rs crates/core/src/pack/markdown.rs crates/app/src/jobs.rs crates/core/src/pack/orchestrator.rs
git commit -m "chore(core): add placeholder per-phase fields to all PackStats literals"
```

---

## Task 3: Wrap each orchestrator phase with `Instant` timing

**Files:**
- Modify: `crates/core/src/pack/orchestrator.rs`

> **What this task does:** Replaces the placeholder zeros from Task 2 Step 7 with actual elapsed-time measurements. The 5 phases match the spec table exactly: walk (walker + pin pre-pass), process (par_iter), secret_scan (loop, optional), tokenize (per-file + per-model, optional), emit (XML/MD/Plain rendering).

- [ ] **Step 1: Add a `walk_start` timer at the start of the walk phase**

In `crates/core/src/pack/orchestrator.rs`, find the line that calls `IgnoreMatcher::new(&root, ...)` (around line 42). Immediately *before* it, add:

```rust
    let walk_start = Instant::now();
```

- [ ] **Step 2: Capture `walk_ms` at the end of the walk phase (after the pin pre-pass closes)**

The walk phase covers `IgnoreMatcher::new` → `walker::walk` → the entire pin pre-pass. The pin pre-pass closes at the comment line `// ── End pin pre-pass ──────────...`. Immediately after that closing comment, add:

```rust
    let walk_ms = walk_start.elapsed().as_millis() as u32;
```

(So `walk_ms` is captured before the `tx.send(ProgressEvent::Walking { ... })` line — the user-visible "walking" progress event reflects actual walk completion.)

- [ ] **Step 3: Time the process phase (the `par_iter`)**

Immediately *before* the `let results: Vec<(FileEntry, Vec<PackWarning>)> = outcome.included.par_iter()` line, add:

```rust
    let process_start = Instant::now();
```

Immediately *after* the `.collect();` that closes the par_iter (around line 201), add:

```rust
    let process_ms = process_start.elapsed().as_millis() as u32;
```

- [ ] **Step 4: Time the secret_scan phase (only when enabled)**

Find the `if opts.secret_scan {` block (around line 268). Replace the entire `if opts.secret_scan { ... }` block with a version that captures elapsed time only when the block runs:

```rust
    let mut secrets_found = 0u32;
    let mut all_redactions: Vec<PackRedaction> = Vec::new();
    let secret_scan_ms: Option<u32> = if opts.secret_scan {
        let secret_scan_start = Instant::now();
        // This loop has two responsibilities:
        //   (1) Build `all_redactions` for the security_report block + PackResult.
        //   (2) Mutate each entry's `content` to its redacted form so the pack
        //       output ships `[REDACTED:<rule-id>]` markers, not the secrets.
        // Hoisting `vendored()` out keeps the keyword-index cache hot across files.
        let ruleset = secrets::ruleset::vendored();
        for e in entries.iter_mut() {
            let result = secrets::scan_and_redact(&e.content, ruleset);
            for r in &result.redactions {
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
            // Replace original content with redacted content so the pack
            // output ships the redacted version, not the secrets.
            e.content = result.redacted_content;
        }
        Some(secret_scan_start.elapsed().as_millis() as u32)
    } else {
        None
    };
```

- [ ] **Step 5: Time the tokenize phase (only when enabled)**

Find the per-file token count block (`if opts.count_tokens { for e in entries.iter_mut() { ... } }` at around line 303) and the `tokens_per_model` computation right below it. Both are part of the tokenize phase. Wrap them together:

```rust
    let tokenize_ms: Option<u32> = if opts.count_tokens {
        let tokenize_start = Instant::now();
        for e in entries.iter_mut() {
            e.tokens = tokens::count_by_name(&opts.tokenizer_model, &e.content).ok();
        }
        Some(tokenize_start.elapsed().as_millis() as u32)
    } else {
        None
    };
```

Then the `tokens_per_model` computation (which currently lives between `bytes_total/tokens_total` accumulation and the stats construction) needs to fold its time into `tokenize_ms` too. Restructure as follows. Replace the existing block:

```rust
    let mut bytes_total = 0u64;
    let mut tokens_total: u32 = 0;
    for e in &entries {
        bytes_total += e.bytes;
        if let Some(t) = e.tokens {
            tokens_total += t;
        }
    }

    let tokens_per_model = if opts.count_tokens {
        let joined: String = entries
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        Some(tokens::count_all(&joined).unwrap_or_default())
    } else {
        None
    };
```

with this version that re-uses the same `tokenize_ms` variable (extending it with the `tokens_per_model` work):

```rust
    let mut bytes_total = 0u64;
    let mut tokens_total: u32 = 0;
    for e in &entries {
        bytes_total += e.bytes;
        if let Some(t) = e.tokens {
            tokens_total += t;
        }
    }

    let (tokens_per_model, tokenize_ms) = if opts.count_tokens {
        let per_model_start = Instant::now();
        let joined: String = entries
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let per_model = Some(tokens::count_all(&joined).unwrap_or_default());
        // Add per-model time to the per-file time captured above.
        let extra = per_model_start.elapsed().as_millis() as u32;
        let total = tokenize_ms.map(|prev| prev + extra).unwrap_or(extra);
        (per_model, Some(total))
    } else {
        (None, None)
    };
```

(The shadowing of `tokenize_ms` here is intentional — the second binding adds the `count_all` time to whatever the per-file loop already accumulated. Be sure to remove the standalone `tokens_per_model` block that was there originally; only the merged version stays.)

- [ ] **Step 6: Time the emit phase**

Find the `let output = match opts.format { ... };` block (around line 367). Wrap it:

```rust
    let emit_start = Instant::now();
    let output = match opts.format {
        PackFormat::Xml => {
            let protocol_block = protocol::block_for_pack(&opts.goal, &opts.protocol_version)?;
            let mut builder = XmlBuilder::new();
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
```

> **Note:** the `match` references `&stats`, but `stats` is constructed *after* this block in the current code. We need to move `stats` construction *before* the match (so the timer wraps just the rendering). See Step 7.

- [ ] **Step 7: Move the `PackStats { ... }` construction to before the emit phase, with all per-phase fields populated**

In the current code, `let stats = PackStats { ... }` lives between the `tokens_per_model` block and the `let dir_paths` line (around line 354). With the changes from Steps 1–6, `stats` is now needed *before* `let output = ...` (the emit match references `&stats`). Move the stats construction up, and populate the new fields:

```rust
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
        emit_ms: 0, // will be overwritten below after emit phase completes
    };

    let dir_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();

    let emit_start = Instant::now();
    let output = match opts.format {
        // ... same body as Step 6 ...
    };
    let emit_ms = emit_start.elapsed().as_millis() as u32;

    // Update emit_ms after the emit phase completes. duration_ms is also
    // refreshed so it captures the emit time too.
    let stats = PackStats {
        emit_ms,
        duration_ms: start.elapsed().as_millis() as u32,
        ..stats
    };
```

(The double-construction is the cleanest way to keep `&stats` available to the renderer — which uses it for the stats block — while still recording the post-emit time. The `..stats` syntax is functional-update of the previous `stats`.)

- [ ] **Step 8: Run the workspace tests — existing tests should still pass with real timings**

```
cargo test --workspace --tests
```

Expected: all tests pass. Per-phase values are now real (small but non-zero on the integration test fixtures).

- [ ] **Step 9: Commit**

```
git add crates/core/src/pack/orchestrator.rs
git commit -m "feat(core/pack): wire per-phase Instant timing into PackStats"
```

---

## Task 4: Add unit tests for per-phase timing

**Files:**
- Modify: `crates/core/src/pack/orchestrator.rs`

- [ ] **Step 1: Add 3 new tests inside the existing `#[cfg(test)] mod tests` block**

Append at the bottom of the `mod tests` block (after `read_text_with_fallback_errors_on_missing_file`):

```rust
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
            &opts,
            tx,
            "job-test",
            CancellationToken::new(),
        )
        .unwrap();
        // walk_ms, process_ms, emit_ms are u32 (always populated). They may
        // be zero on a tiny fixture if the phase finished in <1ms, so we
        // only assert they're set (which is true by virtue of compiling).
        let _: u32 = result.stats.walk_ms;
        let _: u32 = result.stats.process_ms;
        let _: u32 = result.stats.emit_ms;
        // secret_scan_ms and tokenize_ms must be Some(_) when their flags are on.
        assert!(
            result.stats.secret_scan_ms.is_some(),
            "secret_scan_ms must be Some when secret_scan is enabled, got None"
        );
        assert!(
            result.stats.tokenize_ms.is_some(),
            "tokenize_ms must be Some when count_tokens is enabled, got None"
        );
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
            &opts,
            tx,
            "job-test",
            CancellationToken::new(),
        )
        .unwrap();
        assert!(
            result.stats.secret_scan_ms.is_none(),
            "secret_scan_ms must be None when secret_scan is off, got {:?}",
            result.stats.secret_scan_ms
        );
        // sanity: tokenize_ms still populated since count_tokens is on.
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
            &opts,
            tx,
            "job-test",
            CancellationToken::new(),
        )
        .unwrap();
        assert!(
            result.stats.tokenize_ms.is_none(),
            "tokenize_ms must be None when count_tokens is off, got {:?}",
            result.stats.tokenize_ms
        );
        assert!(result.stats.secret_scan_ms.is_some());
    }
```

- [ ] **Step 2: Run the new tests**

```
cargo test -p projectpacker-core --lib pack::orchestrator::tests::pack_populates_per_phase
cargo test -p projectpacker-core --lib pack::orchestrator::tests::pack_omits_secret
cargo test -p projectpacker-core --lib pack::orchestrator::tests::pack_omits_tokenize
```

Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```
git add crates/core/src/pack/orchestrator.rs
git commit -m "test(core/pack): add per-phase timing assertions to orchestrator"
```

---

## Task 5: Regenerate TypeScript bindings

**Files:**
- Auto-generated: `frontend/src/bindings/index.ts`

- [ ] **Step 1: Regenerate**

From the repo root:

```
cargo run -p projectpacker-app --bin emit-bindings
```

Verify the regenerated `frontend/src/bindings/index.ts` contains the new `PackStats` fields:

```
grep -E "walkMs|processMs|secretScanMs|tokenizeMs|emitMs" frontend/src/bindings/index.ts
```

Expected: 5 matches showing all fields exist (camelCased, since `PackStats` uses `serde(rename_all = "camelCase")`).

- [ ] **Step 2: Run frontend typecheck — confirm no breakage**

```
cd frontend && pnpm typecheck
```

Expected: 0 errors. The new fields are all optional from the TypeScript perspective only via `Option<u32>` → `number | null`; `u32` fields → `number`.

- [ ] **Step 3: Commit**

```
git add frontend/src/bindings/index.ts
git commit -m "chore(bindings): regenerate for per-phase PackStats fields"
```

---

## Task 6: Create the `useDragDrop` hook

**Files:**
- Create: `frontend/src/lib/use-drag-drop.ts`

- [ ] **Step 1: Write the hook**

Create `frontend/src/lib/use-drag-drop.ts` with the following content:

```ts
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { stat } from "@tauri-apps/plugin-fs";
import { useEffect, useState } from "react";

/**
 * Resolve a dropped path into a folder path.
 *
 * - If the path is a directory, returns it unchanged.
 * - If the path is a file, returns its parent directory.
 * - If `stat()` fails (race / permission), returns the path as-is and lets
 *   the orchestrator surface any error.
 */
async function resolveFolderPath(path: string): Promise<string> {
  try {
    const info = await stat(path);
    if (info.isDirectory) return path;
  } catch {
    // stat failed — fall through to path-as-is.
    return path;
  }
  // It's a file. Use the parent directory. Cross-platform split on both
  // separators so Windows paths (`C:\foo\bar.txt`) and POSIX paths
  // (`/foo/bar.txt`) both work.
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (idx <= 0) return path;
  return path.slice(0, idx);
}

interface UseDragDropOptions {
  /** Called with the resolved folder path when the user drops a folder/file. */
  onDrop: (folderPath: string) => void;
}

/**
 * Hook that wires Tauri 2's webview-level drag-drop event to React state.
 *
 * Returns `{ isDragging }` so consumers can show a drop overlay while a
 * drag is hovering the window. The `onDrop` callback fires with the first
 * dropped path resolved to a folder (parent dir for files).
 */
export function useDragDrop({ onDrop }: UseDragDropOptions): { isDragging: boolean } {
  const [isDragging, setIsDragging] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const webview = getCurrentWebview();
      unlisten = await webview.onDragDropEvent((event) => {
        const t = event.payload.type;
        if (t === "enter" || t === "over") {
          setIsDragging(true);
        } else if (t === "leave") {
          setIsDragging(false);
        } else if (t === "drop") {
          setIsDragging(false);
          const paths = event.payload.paths ?? [];
          if (paths.length === 0) return;
          if (paths.length > 1) {
            // eslint-disable-next-line no-console
            console.warn(
              `[useDragDrop] ${paths.length} paths dropped; using first: ${paths[0]}`,
            );
          }
          resolveFolderPath(paths[0]).then(onDrop);
        }
      });
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [onDrop]);

  return { isDragging };
}
```

- [ ] **Step 2: Run typecheck — verify the hook compiles**

```
cd frontend && pnpm typecheck
```

Expected: 0 errors.

- [ ] **Step 3: Commit**

```
git add frontend/src/lib/use-drag-drop.ts
git commit -m "feat(frontend): add useDragDrop hook for full-window folder drop"
```

---

## Task 7: Wire `useDragDrop` + `DropOverlay` into `Pack.tsx`

**Files:**
- Modify: `frontend/src/routes/Pack.tsx`

- [ ] **Step 1: Import the hook**

At the top of `frontend/src/routes/Pack.tsx`, add the import alongside the existing imports:

```ts
import { useDragDrop } from "../lib/use-drag-drop";
```

- [ ] **Step 2: Add a `DropOverlay` sub-component**

In the same file, alongside the other sub-components (e.g. `Toggle`, `CopyButton`), add:

```tsx
function DropOverlay({ visible }: { visible: boolean }) {
  if (!visible) return null;
  return (
    <div
      // pointer-events-none lets the underlying webview still receive the
      // drop event; the overlay is purely visual.
      className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-emerald-500/10 backdrop-blur-sm"
    >
      <div className="rounded-lg border-2 border-dashed border-emerald-400 bg-zinc-900/90 px-8 py-6 text-lg font-semibold text-emerald-300 shadow-2xl">
        Drop folder to pack
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Wire the hook inside the `Pack()` component**

Inside the `Pack()` function body, near the other `useState` / `useApp` hooks, add:

```tsx
  const { isDragging } = useDragDrop({
    onDrop: (folderPath: string) => {
      // Auto-switch from GitHub mode to Folder mode if needed, then set value.
      setOptions({
        ...options,
        target: { kind: "folder", value: folderPath },
      });
    },
  });
```

- [ ] **Step 4: Render `<DropOverlay visible={isDragging} />` at the top of the component's returned JSX**

Inside `Pack()`'s `return ( ... )`, add `<DropOverlay visible={isDragging} />` as the first child of the outermost `<div>` (or wherever it makes sense visually — it's `fixed`-positioned so its DOM location does not matter, but conventional location is just inside the outer wrapper):

```tsx
  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <DropOverlay visible={isDragging} />
      {/* existing content unchanged */}
      ...
    </div>
  );
```

- [ ] **Step 5: Run typecheck**

```
cd frontend && pnpm typecheck
```

Expected: 0 errors.

- [ ] **Step 6: Commit**

```
git add frontend/src/routes/Pack.tsx
git commit -m "feat(frontend): wire useDragDrop + DropOverlay into Pack screen"
```

---

## Task 8: Add `PhaseBreakdown` component to result panel

**Files:**
- Modify: `frontend/src/routes/Pack.tsx`

- [ ] **Step 1: Add the `PhaseBreakdown` sub-component**

Alongside `StatsBar` (and `DropOverlay` from Task 7), add a new component:

```tsx
function PhaseBreakdown({ stats }: { stats: PackStats }) {
  // Helpers — render `Some(n)` as `Nms` and `None` as an em-dash.
  const opt = (n: number | null | undefined): string =>
    typeof n === "number" ? `${n}ms` : "—";
  const req = (n: number): string => `${n}ms`;

  return (
    <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 px-1 text-xs font-mono text-zinc-500">
      <span>walk {req(stats.walkMs)}</span>
      <span>· process {req(stats.processMs)}</span>
      <span>· secret-scan {opt(stats.secretScanMs)}</span>
      <span>· tokenize {opt(stats.tokenizeMs)}</span>
      <span>· emit {req(stats.emitMs)}</span>
    </div>
  );
}
```

- [ ] **Step 2: Render `<PhaseBreakdown stats={result.stats} />` directly below `<StatsBar stats={result.stats} />`**

In the `Pack()` component's `return`, find the line that renders `<StatsBar stats={result.stats} />` (inside the `{isDone && result && ( ... )}` block). Add the `PhaseBreakdown` immediately after it:

```tsx
            <StatsBar stats={result.stats} />
            <PhaseBreakdown stats={result.stats} />
```

- [ ] **Step 3: Run typecheck**

```
cd frontend && pnpm typecheck
```

Expected: 0 errors. The `walkMs`, `processMs`, `secretScanMs`, `tokenizeMs`, `emitMs` fields are present on `PackStats` from the regenerated bindings.

- [ ] **Step 4: Commit**

```
git add frontend/src/routes/Pack.tsx
git commit -m "feat(frontend): add per-phase timing breakdown row to result panel"
```

---

## Task 9: CHANGELOG + final verification + push

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add a new `## [Unreleased]` section to CHANGELOG (or extend if one exists)**

Open `CHANGELOG.md`. If a `## [Unreleased]` section already exists at the top, extend its `### Added` / `### Changed` blocks. Otherwise, insert a new section at the top (above the most recent version entry):

```markdown
## [Unreleased]

### Added
- Drag-and-drop folder selection — drop a folder anywhere on the app window to set it as the pack target. Files are resolved to their parent directory; multi-drop takes the first.
- Per-phase timing in `PackStats`: `walk_ms`, `process_ms`, `secret_scan_ms` (optional), `tokenize_ms` (optional), `emit_ms`. Surfaced as an inline breakdown row in the result panel; gives evidence for future perf decisions without doing premature optimization.

### Changed
- Pack screen auto-switches from GitHub URL mode to Folder mode when a folder is dropped onto the window.
```

- [ ] **Step 2: Run the full Rust workspace test suite**

```
cargo test --workspace --tests
```

Expected: all tests pass. Should be at least 180 tests (177 baseline + 3 new from Task 4).

- [ ] **Step 3: Run frontend typecheck**

```
cd frontend && pnpm typecheck
```

Expected: 0 errors.

- [ ] **Step 4: Run a clean cargo build to catch any warnings**

```
cargo build --workspace
```

Expected: no errors. Warnings about `dead_code` for new test helpers are acceptable.

- [ ] **Step 5: Commit CHANGELOG**

```
git add CHANGELOG.md
git commit -m "docs(changelog): v0.4 polish — drag-and-drop + per-phase timing"
```

- [ ] **Step 6: Push to GitHub (origin/main)**

```
git push origin main
```

The user pre-authorized this push.

- [ ] **Step 7: Print summary of what was verified vs deferred**

The autonomous run completed:
- `cargo test --workspace --tests` ✅
- `cargo build --workspace` ✅
- `cd frontend && pnpm typecheck` ✅
- Manual UI smoke test (drag a real folder onto the window) — **deferred to user**: the autonomous environment cannot interactively drag a folder. The hook compiles, the bindings flow through, and the visual overlay JSX is wired; verification of "does dropping actually fire the event in the running Tauri app" requires the user to launch `pnpm tauri dev` and try it.

---
