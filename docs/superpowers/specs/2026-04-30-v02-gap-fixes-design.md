# v0.2 Gap Fixes — Design Spec

**Date:** 2026-04-30
**Scope:** Close the three known gaps from v0.1.0: GitHub URL packing, encoding fallback, and silent failure path.
**Goal:** Make ProjectPacker production-ready for both local folders and public GitHub repositories, with proper error surfacing and broad encoding support.

---

## 1. Problem Statement

v0.1.0 ships with three intentional gaps documented in `CHANGELOG.md`:

1. **GitHub URL packing rejected** — `github.rs` has working URL parsing and shallow clone, but `commands.rs::pack_start` rejects all GitHub targets with `"not_implemented"`.
2. **Encoding fallback limited** — `read_text()` tries UTF-8 then UTF-16LE only. Files in UTF-16BE, Windows-1252, or Latin-1 either decode incorrectly or get mangled silently.
3. **Silent failure path** — `warnings: Vec<PackWarning>` is initialized in the orchestrator and never populated. File read errors, encoding fallbacks, and orchestrator failures are swallowed without surfacing to the UI. `WarningKind::EncodingFallback` and `FileSkipped` are defined but never emitted.

Public-only GitHub support is in scope (private repos / auth deferred).

---

## 2. Architecture Changes

### 2.1 `pack()` signature change

```rust
// Before
pub fn pack(root: &Path, opts: &PackOptions, tx: Sender<PackEvent>, job_id: &str) -> CoreResult<PackResult>

// After
pub fn pack(target: &PackTarget, opts: &PackOptions, tx: Sender<PackEvent>, job_id: &str) -> CoreResult<PackResult>
```

**Rationale:** Move all target resolution into `core`. The app layer (`commands.rs`) should not know how to clone GitHub repos; it should pass the user's intent through and let core resolve it.

### 2.2 New private helper: `resolve_target()`

```rust
fn resolve_target(
    target: &PackTarget,
    job_id: &str,
    tx: &Sender<PackEvent>,
) -> CoreResult<(PathBuf, String, Option<github::ClonedRepo>)>
```

Returns `(root, label, clone_guard)`:
- **Folder:** `(p.clone(), p.display().to_string(), None)`
- **GitHub:**
  1. `tx.send(ProgressEvent::Cloning { progress_pct: 0 })`
  2. `parsed = github::parse_github_url(url)?`
  3. `label = format!("github.com/{}/{}", parsed.owner, parsed.repo)`
  4. `cloned = github::shallow_clone(url, job_id)?`
  5. Return `(cloned.path.clone(), label, Some(cloned))`

The `Option<ClonedRepo>` is held in scope by `pack()` as `_clone_guard`. When `pack()` returns, the `TempDir` inside `ClonedRepo` is dropped and the temp directory is deleted automatically.

### 2.3 `commands.rs::pack_start` simplified

- **Remove** the `not_implemented` GitHub rejection block (lines 58–69).
- **Remove** the manual `match &opts.target` → root extraction (lines 81–84).
- **Replace** the call site:

```rust
match pack::pack(&opts.target, &opts, tx.clone(), &id_for_task) {
    Ok(result) => registry_for_task.store_result(&id_for_task, result),
    Err(e) => {
        let _ = tx.send(ProgressEvent::Error {
            message: e.to_string(),
            fatal: true,
        });
    }
}
```

The send-then-drop pattern (currently `let _ = tx.send(...)`) is preserved for the error event because the channel may already be closed if the UI navigated away.

---

## 3. Encoding Chain

### 3.1 New helper: `read_text_with_fallback()`

Replaces the existing `read_text()`. Returns `(content, used_fallback)`:

```rust
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
```

**Rationale:** BOM-first detection is the canonical Microsoft/web approach. Windows-1252 as final fallback covers Latin-1 and most Western European single-byte encodings; it's a strict superset of ISO-8859-1 and never fails to decode.

---

## 4. Warning Collection

### 4.1 par_iter map shape change

The parallel iterator that builds `entries` changes its return type from `FileEntry` to `(FileEntry, Vec<PackWarning>)`. After collection, a single sequential loop splits the results.

```rust
let results: Vec<(FileEntry, Vec<PackWarning>)> = outcome
    .included
    .par_iter()
    .map(|f| {
        let abs = root.join(&f.path);
        let mut file_warnings = Vec::new();

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

        // ... existing comment removal / compression / token counting / hashing ...

        (entry, file_warnings)
    })
    .collect();

let mut warnings: Vec<PackWarning> = Vec::new();
let mut entries: Vec<FileEntry> = Vec::with_capacity(results.len());
for (entry, w) in results {
    entries.push(entry);
    warnings.extend(w);
}
```

`warnings` is then included in the returned `PackResult`.

### 4.2 Tree-sitter failures

`compress()` and `remove_comments()` currently return `String` (infallible). They are not wrapped in `catch_unwind` for v0.2 — tree-sitter is stable in practice and adding panic recovery for every file in a parallel iterator is more complexity than the rare panic justifies. If a real-world panic surfaces, we add `catch_unwind` in a follow-up.

`WarningKind::TreeSitterFailed` is defined but stays unused for v0.2.

---

## 5. Frontend Changes

### 5.1 Target mode toggle

In `frontend/src/routes/Pack.tsx`:

- Add UI state derived from `options.target.kind`: `"folder" | "github"`
- Two-button toggle group above the existing target row:
  ```
  [ Folder ] [ GitHub URL ]
  ```
- **Folder mode:** existing folder picker (unchanged)
- **GitHub mode:** text input with placeholder `https://github.com/owner/repo`
  - Validation regex: `^(https:\/\/github\.com\/|git@github\.com:|github\.com\/)[^\/\s]+\/[^\/\s]+\/?$`
  - On invalid: red border + small inline error text "Enter a valid GitHub repo URL"
- Pack button disabled until target is valid for the active mode (folder picked, or URL passes regex)

Switching modes resets the target value to `{ kind: <mode>, value: "" }`.

### 5.2 ProgressLog updates

- New case for `ProgressEvent::Cloning { progress_pct }`:
  ```
  Cloning repository…
  ```
  Color: blue/cyan (distinct from walking/scanning events).
- Rename `BuildingXml` case → `BuildingOutput`. Label changes from `"Building XML…"` to `"Building output…"`.

### 5.3 Warnings panel

Already exists in Pack.tsx. No changes — will populate automatically once orchestrator emits warnings. Verify it renders both `FileSkipped` and `EncodingFallback` kinds with sensible labels.

### 5.4 Error handling

Already exists. The store's `pushEvent` sets `status: "error"` on `ProgressEvent::Error`, which the UI surfaces. The new fatal-error events from `commands.rs` will flow through this existing path.

---

## 6. Type Changes & Bindings

### 6.1 `types.rs`

- **Rename** `ProgressEvent::BuildingXml` → `ProgressEvent::BuildingOutput`. Variant has no fields, no migration needed beyond a search-and-replace in one orchestrator.rs site and one Pack.tsx case.
- All other types unchanged. `WarningKind`, `PackWarning`, `ProgressEvent::Cloning` already exist.

### 6.2 Bindings regeneration

Run `cargo run -p projectpacker-app --bin emit-bindings` after Rust changes to update `frontend/src/bindings/index.ts`. Verify TypeScript still typechecks; update Pack.tsx if any switch statement breaks.

---

## 7. Testing Strategy

### 7.1 Unit tests

**`crates/core/src/pack/orchestrator.rs`** (in existing `mod tests`):

- `read_text_with_fallback` — write 5 fixture files in a temp dir:
  1. Plain UTF-8 → returns content, `false`
  2. UTF-8 with BOM → BOM stripped, returns content, `false`
  3. UTF-16 LE with BOM → returns content, `true`
  4. UTF-16 BE with BOM → returns content, `true`
  5. Windows-1252 (e.g. `\xE9` for é) → decodes correctly, `true`

- `pack_emits_encoding_fallback_warning` — fixture with one UTF-16LE file, assert `result.warnings` contains an `EncodingFallback` entry for that path.

- `pack_emits_file_skipped_warning_on_unreadable` — fixture where one file becomes unreadable mid-pack (skip on platforms where this is hard to simulate; gate behind `cfg(unix)` if needed).

- `pack_resolves_folder_target` — call `pack(&PackTarget::Folder(d.path().into()), ...)` and verify it produces output identical to the previous `pack(d.path(), ...)` flow.

**`crates/core/tests/pack_integration.rs`:**

- Update existing 3 tests to use `&PackTarget::Folder(root)` instead of `&root`.

### 7.2 Integration test (GitHub)

- **No live network test.** A test that clones from real GitHub is flaky and slow. Instead:
- Existing `github::parse_github_url` tests cover the parse path.
- Existing `github::shallow_clone` is unit-testable only with network; we trust it (it was tested manually in v0.1.0).
- Add `pack_target_github_resolution_emits_cloning_event` — gated behind a feature flag or `#[ignore]`, run manually before release.

### 7.3 Frontend

- TypeScript typecheck: `pnpm typecheck` must pass after binding regeneration.
- Manual smoke test: paste a small public GitHub URL (e.g. `https://github.com/octocat/Hello-World`) and verify:
  1. Cloning event appears in ProgressLog
  2. Files are packed
  3. Output is non-empty
  4. Stats show the expected file count

---

## 8. Out of Scope (deferred to v0.3+)

- Private GitHub repos / GitHub PAT in settings
- Drag-and-drop folder targeting
- Git history embedding (`include_git_history` option)
- Tree-sitter panic recovery (`WarningKind::TreeSitterFailed`)
- Secret scan failure surfacing (`WarningKind::SecretScanFailed`)
- Cancellation propagation through clone (cancelling during clone leaves the clone running until completion)

---

## 9. Migration & Rollout

This is a single-PR change with breaking API changes only inside the workspace (no external consumers of the Rust crate). Frontend bindings regeneration is mechanical. CHANGELOG `[Unreleased]` section gets new entries:

- **Added:** GitHub public repo packing, BOM-aware encoding fallback chain (UTF-8 / UTF-16LE / UTF-16BE / Windows-1252), `EncodingFallback` and `FileSkipped` warnings, fatal error surfacing from orchestrator.
- **Changed:** `pack()` now takes `&PackTarget` instead of `&Path`. `ProgressEvent::BuildingXml` renamed to `BuildingOutput`.
