# v0.6 — Lossless Compression Pipeline (Design Spec)

**Status:** Draft (approved 2026-05-11)
**Owner:** CBaileyDev
**Predecessors:** v0.5 (current tip `2282b2d`)

## 1. Goal

Compress pack output as aggressively as possible *without losing information or quality* the consumer AI would actually use. Expose every transform as an individual toggle in the UI; default ON only the transforms that are *cosmetically lossless* (a reader literally cannot distinguish the output from the original). Surface per-transform savings so users can see what their settings bought them.

This work is the v0.6 milestone. v0.7+ will tackle pack-scope correctness (what gets included) and UI polish — out of scope here.

## 2. Scope summary

| Bucket | Transforms | Default |
|---|---|---|
| **Lossless** | `dedup_files`, `trim_trailing_ws`, `collapse_blank_lines`, `normalize_line_endings` | **ON** |
| **Semantic** | `collapse_lockfiles`, `collapse_minified`, `mark_generated` | OFF |
| **Code shaping (lossy)** | `compress` (skeleton), `remove_comments`, `elide_type_only_exports` | OFF |

10 toggles total. `compress` and `remove_comments` already exist; the other 8 are new.

## 3. Architecture

A new `transform` phase inserts into the existing pack pipeline:

```
walk → process → [NEW: transform] → pin-reorder → secret-scan → tokenize → emit
```

- The transform phase lives in `crates/core/src/transforms/`. Each transform is its own module file.
- `run_transform_phase(&mut entries, &opts, throttler) -> (Vec<TransformReport>, u32)` walks the transforms in fixed order. Each transform short-circuits if its toggle is OFF (no cost paid for disabled transforms).
- Transforms produce a `TransformReport { id, bytes_saved, files_touched, elapsed_ms }`. Reports aggregate into `PackStats.transforms`.
- Per-file transforms run inside `par_iter_mut` (matching the existing `process_phase` style); cross-file dedup runs as a serial pass over the existing `FileEntry.hash` field (already BLAKE3, already computed).
- Lossless transforms run *before* lossy ones so `tokens_total` reflects the actual emitted text.

File layout:

```
crates/core/src/transforms/
  ├─ mod.rs              ← run_transform_phase + TransformReport
  ├─ dedup.rs            ← cross-file BLAKE3 grouping
  ├─ normalize.rs        ← trim_trailing_ws + collapse_blank_lines + normalize_line_endings
  ├─ collapse_lockfile.rs
  ├─ collapse_minified.rs
  ├─ mark_generated.rs
  ├─ compress_skeleton.rs ← thin wrapper around tree_sitter_compress::compress
  ├─ strip_comments.rs    ← thin wrapper around tree_sitter_compress::remove_comments
  └─ elide_types.rs       ← TS-only tree-sitter pass
```

`tree_sitter_compress.rs` is unchanged. Existing call sites in `orchestrator::run_process_phase` that invoke `compress` and `remove_comments` move into the new module structure, gated by the new transform toggles.

## 4. The 10 transforms

### 4.1 `dedup_files` (default ON, cross-file)

Group `FileEntry` values by `hash`. For each group with >1 member, sort members lexicographically by `path` ascending; the first occurrence in that sorted order keeps its content unchanged (regardless of the walker's emission order). Subsequent occurrences:

- XML emit: `<document path="<path>" duplicate-of="<first_path>" sha="<12-char>" />` (empty body, self-closing).
- MD/Plain emit: body replaced with `[DUPLICATE OF: <first_path> | sha: <12-char>]`.
- `bytes_saved = sum(original bytes of all non-first duplicates)`.
- `files_touched = count of non-first duplicates`.

### 4.2 `trim_trailing_ws` (default ON, per-file)

For each file, replace every line's trailing run of spaces/tabs with empty. Preserves intentional indentation. `bytes_saved` measured against pre-transform bytes.

### 4.3 `collapse_blank_lines` (default ON, per-file)

Runs of ≥3 blank lines collapse to exactly 2. Single and double blank-line separations preserved. Implemented as a scan over `str::lines()` with a run counter.

### 4.4 `normalize_line_endings` (default ON, per-file)

`\r\n` → `\n`; lone `\r` → `\n`. Idempotent. Implemented via two `str::replace` calls.

### 4.5 `collapse_lockfiles` (default OFF, per-file)

Match by basename: `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `Cargo.lock`, `Gemfile.lock`, `poetry.lock`, `composer.lock`, `Pipfile.lock`, `go.sum`. On match, body becomes:

```
<first 20 lines of original>
[COMPRESSED: lockfile | original-bytes: N | sha: <12-char>]
[N lines omitted]
<last 5 lines of original>
```

### 4.6 `collapse_minified` (default OFF, per-file)

Detect via:
- **At least one line > 2000 characters**, AND
- **If the file has > 5 lines**: additionally require `avg_line_length / median_line_length > 5`.

The avg/median check suppresses false positives on tab-aligned data files (which have many long lines but consistent length). For files with ≤5 lines a single long line is enough — true minified bundles often emit as a single line and would otherwise be missed. On match, body becomes:

```
<first 200 chars of original>
[MINIFIED BUNDLE: N bytes | sha: <12-char>]
<last 100 chars of original>
```

### 4.7 `mark_generated` (default OFF, per-file)

Detect via *either*:
- **Banner match** in the first 2 KB: `@generated`, `DO NOT EDIT`, `Code generated by`, `This file is automatically generated`, `AUTO-GENERATED`. Match is whole-substring, case-sensitive except `DO NOT EDIT` (case-insensitive — common variants exist).
- **Filename pattern**: `*.pb.go`, `*.gen.ts`, `*.gen.rs`, `bindings/index.ts`, `*_pb2.py`, `*.pb.cc`.

When the banner matched, the "detected banner line" is the first matched banner's line; when only the filename pattern matched, it's the file's first non-blank line. On match, replace body with:

```
<the detected banner line, verbatim>
[GENERATED FILE — body suppressed | sha: <12-char>]
```

### 4.8 `compress` (default OFF, per-file) — existing

Renamed to "Skeleton-compress functions" in the UI; field name unchanged for preset compatibility. Calls `tree_sitter_compress::compress` for `.rs`/`.py`/`.js`/`.ts`/`.jsx`/`.tsx`/`.mjs` files. Now reports `bytes_saved` like every other transform.

### 4.9 `remove_comments` (default OFF, per-file) — existing

UI label unchanged. Calls `tree_sitter_compress::remove_comments`. Now reports `bytes_saved`.

### 4.10 `elide_type_only_exports` (default OFF, per-file)

TypeScript only (`.ts`, `.tsx`). Tree-sitter query identifies `export type { … } from "…"` re-export lines where every name is a type-only re-export. Removes those lines. Leaves `export type Foo = …` declarations untouched (those are definitions, not re-exports).

## 5. Data model

### 5.1 `PackOptions` (`crates/core/src/types.rs`)

8 new fields with `#[serde(default = …)]` so old presets deserialize cleanly:

```rust
pub struct PackOptions {
    // ... existing fields unchanged ...
    pub remove_comments: bool,
    pub compress: bool,

    #[serde(default = "default_true")] pub dedup_files: bool,
    #[serde(default = "default_true")] pub trim_trailing_ws: bool,
    #[serde(default = "default_true")] pub collapse_blank_lines: bool,
    #[serde(default = "default_true")] pub normalize_line_endings: bool,
    #[serde(default)] pub collapse_lockfiles: bool,
    #[serde(default)] pub collapse_minified: bool,
    #[serde(default)] pub mark_generated: bool,
    #[serde(default)] pub elide_type_only_exports: bool,
}

fn default_true() -> bool { true }
```

Update `Default for PackOptions` accordingly.

### 5.2 New types

```rust
pub struct TransformReport {
    pub id: String,             // "dedup_files", "trim_trailing_ws", ...
    pub bytes_saved: u64,
    pub files_touched: u32,
    pub elapsed_ms: u32,
}
```

### 5.3 `PackStats` additions

```rust
pub struct PackStats {
    // ... existing fields ...
    pub transforms: Vec<TransformReport>,   // one entry per ENABLED transform, in execution order
    pub transform_phase_ms: u32,            // total phase elapsed
}
```

`transforms` is empty when no transform toggle is on; the renderer omits `<compression_report>` in that case.

### 5.4 New `ProgressEvent` variants

```rust
ProgressEvent::TransformStart { id: String }
ProgressEvent::TransformDone  { id: String, bytes_saved: u64, files_touched: u32 }
```

Throttler passes these through (low frequency — 10 events max per pack).

## 6. UI

### 6.1 Placement

A collapsible "Compression" disclosure on the Pack screen (Option B-1). Header row shows `▸ Compression — N of 10 enabled, X saved last run` and expands to reveal the grouped list.

### 6.2 Layout

```
Compression                                      Last run: 1.2 MB saved   ▾

LOSSLESS — applied by default                              ✓ all on
  ●━━  Dedup duplicate files                          1.2 MB saved
  ●━━  Trim trailing whitespace                          24 KB saved
  ●━━  Collapse blank lines                              18 KB saved
  ●━━  Normalize line endings (CRLF → LF)                 0 B saved

SEMANTIC — opt-in, no information loss                   ✗ all off
  ━━○  Collapse lockfiles                                    —
  ━━○  Collapse minified bundles                             —
  ━━○  Mark generated files                                  —

CODE SHAPING — opt-in, modifies code (lossy)             ✗ all off
  ━━○  Skeleton-compress functions                           —
  ━━○  Strip comments                                        —
  ━━○  Elide TypeScript type-only re-exports                 —
```

### 6.3 Component reuse

- **Toggle affordance:** existing `Toggle.tsx` (iOS-style switches). No new component.
- **Group chip** (`✓ all on` / `✗ all off` / `~ 2 of 3 on`) is clickable to bulk-toggle the group.
- **Per-row savings number** sourced from `PackStats.transforms` by matching `TransformReport.id` to the row's transform key (not by index — disabled transforms are absent from the Vec). Pre-first-pack: `—`. Updates live during a pack via the new `TransformDone` IPC event.
- **Description tooltip** on a `(i)` icon per row.
- **Disabled state:** if a transform's `files_touched == 0` on the last run, show `n/a — no eligible files` instead of `0 B saved`.
- **Disclosure animation:** existing `motion.ts`.

### 6.4 Styling notes

- Extend Tailwind v4 `@theme` block in `globals.css` with `--color-transform-savings-fg` (subtle green).
- Group section headers: 13px caption + 11px chip.
- Row hover: `bg-surface-2/40` (or nearest existing token).
- Flat rows, not cards.

### 6.5 New frontend files

```
frontend/src/components/pack/
  ├─ CompressionPanel.tsx          ← new: the disclosure + grouped layout
  └─ TransformRow.tsx              ← new: one row with toggle + savings + tooltip
```

`Pack.tsx` mounts `CompressionPanel` where the existing `compress` and `remove_comments` toggles live, replacing them.

## 7. Protocol

**No version bump.** Markers are self-describing English; an AI that's never seen them reads them correctly. Extend `docs/protocol/grok-to-cc-v1.md` with:

```markdown
## Compression markers

The pack may contain placeholders inserted by lossless transforms:

### File-body markers
- `[DUPLICATE OF: <path> | sha: <12-char-prefix>]`
  File is byte-identical to <path>. Consult the named file for content.
- `[COMPRESSED: <reason> | original-bytes: N | sha: <12-char-prefix>]`
  Body collapsed. <reason> ∈ {lockfile, minified, generated}.
  Lockfile/minified bodies retain first/last N lines; generated bodies retain
  the detection banner.

### XML attribute
- `<document path="..." duplicate-of="..." sha="..." />` — same semantic as the
  body marker; used when the body is empty.

### Compression report
Every pack with at least one transform applied emits `<compression_report>`
(or MD/Plain equivalent) listing every applied transform with bytes saved and
files touched.
```

Executor section gains one sentence:

> Do not treat compression markers as missing content. The original content is either available in the duplicate's first occurrence or, if compressed, was deemed low-signal by the user.

## 8. Emit changes

### 8.1 XML (`crates/core/src/pack/xml.rs`)

- `<document>` element gains optional `duplicate-of` and `sha` attributes.
- New `<compression_report>` block emitted between `<security_report>` and `<directory_structure>` when `stats.transforms` is non-empty.
- `<compression_report>` body: one `<transform id="..." bytes_saved="N" files_touched="M" elapsed_ms="K"/>` per applied transform.

### 8.2 Markdown (`crates/core/src/pack/markdown.rs`)

- Files with `[DUPLICATE OF: …]` body emit normally — no schema change needed.
- New "Compression report" section heading after the security-report section, table-formatted: `| Transform | Bytes saved | Files touched | Elapsed (ms) |`.

### 8.3 Plain (`crates/core/src/pack/plain.rs`)

- Same as Markdown, but the report is a key:value listing under a `=== Compression report ===` divider.

## 9. Error handling

Per-transform per-file fallback: if a transform fn errors on any file, the file passes through *that transform* unchanged and a `PackWarning { kind: TransformFailed, path, message }` is emitted. The transform's `TransformReport.files_touched` reflects only successes.

Adds one `WarningKind` variant: `TransformFailed`. Frontend warning display already handles unknown kinds gracefully.

The phase itself cannot fail — every file passes through every disabled-or-errored transform unchanged. Disabling all transforms is functionally a no-op phase.

## 10. Testing

### 10.1 Unit tests

One test module per transform (10 modules). Each covers:
- Toggle OFF → content unchanged, `bytes_saved == 0`, `files_touched == 0`.
- Toggle ON, no eligible content → unchanged, `bytes_saved == 0`, `files_touched == 0`.
- Toggle ON, eligible content → expected output + accurate `bytes_saved` + correct `files_touched`.
- Idempotence: applying twice produces the same result as once.

### 10.2 Dedup specifics

- Two byte-identical files → second becomes marker, first retains content.
- Three duplicates → first retains, second + third reference first.
- Sort stability: when two files share a hash, lexicographic-min-path wins regardless of walker order.

### 10.3 Detection fixtures

- **Minified:** positive: hand-crafted 2 KB single-line bundle. Negatives: 2 KB tab-separated CSV with one long line; normal JS file.
- **Generated:** positive per banner pattern; positive per path pattern; negative: normal source.

### 10.4 Integration tests (`crates/core/tests/pack_integration.rs`)

New fixture `tests/fixtures/compression/`:
- 2 duplicate LICENSE files
- 1 `package-lock.json`
- 1 minified bundle
- 1 generated file (with `@generated` banner)
- 1 plain source

Test pack with all 10 transforms ON; verify:
- Output XML is well-formed.
- `<compression_report>` present, with non-zero `bytes_saved` for each enabled transform that had eligible content.
- Each marker format present at least once in the output.
- Stats fields populate correctly.

### 10.5 Snapshot test (`crates/core/tests/protocol_golden.rs`)

One new `insta` snapshot of the `<compression_report>` block to catch accidental format drift.

### 10.6 Serde compat

A test deserializing a `PackOptions` JSON missing all 8 new fields; assert correct defaults.

### 10.7 Frontend

Add `frontend/src/components/pack/CompressionPanel.test.tsx` (Vitest): renders the panel, asserts the chip toggles all rows in a group, asserts a row click flips the underlying store value. This is the first vitest test in the codebase.

### 10.8 Verification gate

Existing gate, expanded with `pnpm bindings`:

```
cargo test --workspace --tests
cargo clippy --workspace --all-targets -- -D warnings
pnpm bindings
pnpm --dir frontend typecheck
pnpm --dir frontend lint
pnpm --dir frontend test
```

## 11. Rollout phases

Single PR is fine, but the work splits into 6 mechanical phases:

1. **A — Pipeline plumbing.** `transforms/mod.rs`, `TransformReport`, IPC events. Wire into orchestrator with empty transform-module bodies. Phase-contract tests.
2. **B — Lossless 4.** `dedup`, `trim_trailing_ws`, `collapse_blank_lines`, `normalize_line_endings`. Default ON.
3. **C — Semantic 3.** Lockfile, minified, generated. Default OFF. Detection fixtures.
4. **D — Code shaping 3.** Move existing `compress` + `remove_comments` into the new structure. Add `elide_type_only_exports`.
5. **E — UI.** `CompressionPanel`, `TransformRow`. Bindings regenerate. Vitest test.
6. **F — Protocol docs + CHANGELOG + emit changes.** XML/MD/Plain `<compression_report>` block. Marker docs in protocol.

## 12. Out of scope (deferred)

- Near-duplicate detection (Shingling, MinHash).
- Cross-language semantic dedup (identical-by-AST).
- Custom user-defined transforms (plugin API).
- Per-file transform overrides via path patterns (e.g., "don't dedup `vendor/**`").
- Binary archive output (tar/zip) — violates the AI-readable GUI-only constraint.
- Compression for binary files (excluded by walker already).

## 13. Risk register

| Risk | Mitigation |
|---|---|
| AI consumer misreads markers | Plain-English marker format + protocol doc update. Escape hatch: bump to `grok-to-cc-v1.1` later if needed. |
| Minified detection false-positives on data files | Two-signal AND check (long-line *and* high length-variance) guards against single-line-CSV false positives. Fixture-tested. |
| Generated detection false-positives on hand-written code that happens to contain the word "generated" | Banner detection is full-phrase match on 2KB scan, not substring. Path patterns gate the rest. |
| BLAKE3 hash collision | Full 256-bit digest used for comparison; 12-char prefix is display-only. Collision-resistant beyond practical concern. |
| Backward-compat for existing presets | `#[serde(default = …)]` on every new field. Old JSON deserializes cleanly. Tested. |
| Tokens-counted-against-wrong-content drift | Transform phase runs *before* tokenize phase; tokens reflect post-transform bytes. Already the case for existing `compress`/`remove_comments`. |
| Tree-sitter parser construction cost for `elide_type_only_exports` on many small files | Reuse the existing parser pool from `tree_sitter_compress.rs::PooledParser`. The 200-byte small-file skip path does *not* apply to `elide_type_only_exports` because correctly identifying type-only re-exports requires the parse tree even for small files. |

## 14. Open questions resolved

| Question | Decision |
|---|---|
| Strictly lossless vs semantic-lossless vs lossy? | All three classes in scope, gated by individual toggles; default ON only the cosmetic-lossless 4. |
| UI granularity? | Fully granular (10 toggles), grouped into 3 sections. |
| Defaults aggressive or conservative? | Conservative — only the truly-zero-loss transforms ON. |
| Approach (pipeline / trait / fold)? | Pipeline (Approach A). |
| Protocol version bump? | No. Self-describing markers; extend v1 docs. |
| Preserve `compress`/`remove_comments` field names on disk? | Yes — preset compat. UI labels them as "Skeleton-compress functions" and "Strip comments". |
| Per-transform IPC events? | Yes (`TransformStart`/`TransformDone`). |
| Per-transform `elapsed_ms`? | Yes. Single `Instant` per transform is negligible cost. |
