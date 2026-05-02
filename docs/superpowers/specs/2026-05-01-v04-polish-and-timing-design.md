# v0.4 Polish: Drag-and-Drop Folder + Per-Phase Timing — Design

**Status:** Approved (delegated)
**Date:** 2026-05-01
**Author:** Brainstorming session, autonomous execution

---

## Goal

Two focused polish improvements for v0.4:

1. **Drag-and-drop folder selection.** Currently the only way to pick a folder is the Browse dialog or pasting a path. Add full-window drop support so the user can drag a folder from Explorer/Finder and drop it anywhere on the app window.
2. **Per-phase timing in `PackStats`.** Currently `PackStats` only carries `duration_ms` (whole-pack total). Split it into per-phase fields and surface them in the result panel. No optimization — measurement only — so future perf decisions have evidence.

Both items came out of a brainstorm in which the user prioritized polish-and-UX over big new features, then explicitly scoped to drag-and-drop (no keyboard shortcuts) plus speculative perf reframed as measurement.

---

## Architecture

### Backend — Rust core

Wrap each major orchestrator phase with `Instant::now()` / `.elapsed()` and store the result in new `PackStats` fields. Total `duration_ms` stays as the wall-clock total.

#### Phase boundaries (matching the existing orchestrator)

| Phase | What it covers | When it runs | New `PackStats` field |
|---|---|---|---|
| Walk | `walker::walk` + pin pre-pass | Always | `walk_ms: u32` |
| Process | `par_iter` over included files: read + encoding fallback + optional comment-removal + optional compress | Always | `process_ms: u32` |
| Secret scan | Sequential redaction loop (`scan_and_redact` per entry) | Only when `opts.secret_scan == true` | `secret_scan_ms: Option<u32>` |
| Tokenize | Per-file token count + per-model `count_all` on joined content | Only when `opts.count_tokens == true` | `tokenize_ms: Option<u32>` |
| Emit | Final XML / Markdown / Plain text rendering | Always | `emit_ms: u32` |

Skipped phases use `Option::None` (not `Some(0)`) so the UI can render them as `—` rather than misleading "0ms".

#### Why these five phases (not more, not fewer)

- **Pin pre-pass is folded into Walk** — it shares the path-resolution work and runs before any file content is touched.
- **Per-file token count and per-model `count_all` are folded into one Tokenize phase** — they share a tokenizer cache and one happens immediately after the other.
- **Hashing and warning-collection are folded into Process** — they happen inside the same `par_iter` closure.
- **Stats struct construction and `Done` event emission are not measured** — they're sub-millisecond and run after all timing is captured.

### Frontend — React + Tauri

Two pieces, each in its own file:

1. **`useDragDrop` hook** (`frontend/src/lib/use-drag-drop.ts`) — wires Tauri 2's webview-level `onDragDropEvent` to React state. Exposes `{ isDragging, setOnDrop }` to consumers.
2. **`PhaseBreakdown` component** (inline in `Pack.tsx`) — renders below `StatsBar`. Single line, monospace, muted color. Format: `walk 12ms · process 240ms · secret-scan 30ms · tokenize 50ms · emit 8ms` with `—` for skipped.

#### Drag-and-drop interaction model

- **Drop target:** the entire webview (Tauri's window-level event — there is no per-element drop in Tauri 2).
- **Visual:** when dragging, a `pointer-events-none` full-screen overlay (`fixed inset-0 z-50 bg-emerald-500/10 backdrop-blur-sm`) with a centered card that says *"Drop folder to pack"*. Disappears on `leave` or `drop`.
- **Drop processing:**
  - 0 paths → ignore (defensive; shouldn't happen).
  - 1+ paths → take the first; if more than one was dropped, log a `console.warn` and continue silently (no toast — keeps scope tight; can add later).
  - Determine if the path is a folder via `@tauri-apps/plugin-fs::stat()`:
    - **Folder:** use as-is.
    - **File:** use the parent directory (string-manipulate the path; cross-platform via splitting on `/` and `\`).
    - **`stat()` fails** (race / permission): use the path as-is and let the orchestrator surface any error.
  - **Mode auto-switch:** if currently in GitHub URL mode, switch to Folder mode and set the value. If already in Folder mode, replace the value.

#### Phase breakdown UI

- Always visible (no expand/collapse) — discoverability over compactness.
- Position: directly below `StatsBar` and above the copy buttons.
- Tailwind: `text-xs text-zinc-500 font-mono mt-2` with `·` separators.
- Skipped phase: `secret-scan —` (using em-dash so it's visually distinct from a 0).

---

## Data flow

### Drag-drop
```
[OS drag] → Tauri webview event → useDragDrop hook → React state (isDragging)
[OS drop] → useDragDrop hook fires onDrop(paths) → resolveFolderPath(paths[0])
        → setOptions({ ...options, target: { kind: "folder", value: resolved } })
        → input reflects new target
```

### Per-phase timing
```
pack(...)
  start = Instant::now()
  walk_start = Instant::now()
    walker::walk(...) + pin pre-pass
  walk_ms = walk_start.elapsed().as_millis() as u32

  process_start = Instant::now()
    par_iter(...)
  process_ms = process_start.elapsed().as_millis() as u32

  secret_scan_ms = if opts.secret_scan {
    let s = Instant::now(); /* loop */ Some(s.elapsed().as_millis() as u32)
  } else { None };

  tokenize_ms = if opts.count_tokens { /* same */ } else { None };

  emit_start = Instant::now()
    match opts.format { ... }
  emit_ms = emit_start.elapsed().as_millis() as u32

  PackStats { duration_ms: start.elapsed(), walk_ms, process_ms, secret_scan_ms, tokenize_ms, emit_ms, ... }
```

---

## Files affected

### Backend (Rust)

| File | Change |
|---|---|
| `crates/core/src/types.rs` | Add 5 fields to `PackStats` (3 `u32`, 2 `Option<u32>`); update existing `PackStats` literal in test |
| `crates/core/src/pack/orchestrator.rs` | Wrap 5 phases with `Instant::now()`; populate new fields in the `PackStats` constructor at the bottom |
| `crates/core/src/pack/markdown.rs` | Update PackStats constructor sites in tests (no behavioural change to renderer) |
| `crates/core/src/pack/plain.rs` | Same — update PackStats constructor in tests |
| `crates/core/src/pack/xml.rs` | Same — update PackStats constructor in tests; the `stats_block` does NOT need to surface per-phase timing in the pack output (these are UI-only) |
| `crates/core/src/pack/stats.rs` | Update PackStats constructor in tests if any |
| `crates/core/tests/pack_integration.rs` | Update PackStats fixtures if any; add one assertion that the new fields are populated |

### Bindings (auto-generated)

| File | Change |
|---|---|
| `frontend/src/bindings/index.ts` | Regenerated by `cargo run -p projectpacker-app --bin emit-bindings` |

### Frontend (TypeScript / React)

| File | Change |
|---|---|
| `frontend/src/lib/use-drag-drop.ts` | New file — `useDragDrop` hook |
| `frontend/src/routes/Pack.tsx` | Wire `useDragDrop`, add overlay JSX, add `PhaseBreakdown` component below `StatsBar` |
| `CHANGELOG.md` | New `## [Unreleased]` (or extend existing) with v0.4 entries |

---

## Error handling

### Drag-drop
- **Path doesn't exist by the time the user clicks Pack:** existing orchestrator path — `pack()` returns `CoreError::PathNotFound`, surfaced via the existing error pipe.
- **`stat()` fails (race / permission):** fall back to using the dropped path as-is; let the orchestrator handle it.
- **Empty `paths` array:** ignore the drop (no-op).
- **`isDragging` stuck on (race between `enter` and `leave`):** acceptable — the next `drop` or `leave` event will clear it. No timeout fallback in v0.4.

### Per-phase timing
- No new error paths. `Instant::now()` and `Instant::elapsed()` are infallible.
- `as_millis() as u32` saturates correctly for any realistic pack (would need >49 days to overflow `u32` ms).

---

## Testing

### Backend
- **New unit test in `pack/orchestrator.rs`:** `pack_populates_per_phase_timing_fields` — pack a tiny fixture with `secret_scan: true, count_tokens: true` and assert all 5 fields are `Some(_)` (or non-zero `u32`).
- **New unit test:** `pack_omits_secret_scan_ms_when_disabled` — pack with `secret_scan: false`; assert `secret_scan_ms == None`.
- **New unit test:** `pack_omits_tokenize_ms_when_disabled` — same for `count_tokens: false`.
- **Existing test updates:** every `PackStats { ... }` literal in tests must include the new fields. This is mechanical.

### Frontend
- **Manual smoke test (cannot automate without launching the dev server):**
  - Drag a folder from Explorer → window highlights → drop → target field populates with the folder path → mode is "Folder".
  - Drag a single file → drop → target field populates with parent directory.
  - Drag while in GitHub URL mode → drop a folder → mode switches to Folder, value populates.
  - Drag multiple folders → drop → first one wins, no error.
  - Pack a small folder → result panel shows phase breakdown row with all 5 phases populated.
  - Pack a small folder with `secret_scan` off → phase breakdown shows `secret-scan —`.
- **No new Vitest tests** — the hook depends on Tauri's webview event API which is not easy to mock cleanly; the manual smoke covers it.

### Verification gate (autonomous run)
Before pushing, the autonomous run must verify:
1. `cargo test --workspace --tests` — all tests pass.
2. `cargo build --workspace` — clean build.
3. `cd frontend && pnpm typecheck` — 0 errors.
4. **Manual UI smoke is deferred to the user** — explicitly noted in the final summary.

---

## Out of scope (for v0.4 — explicitly deferred)

- Multiple-folder drop (currently first-only, no toast).
- File drop with explicit error message (silently uses parent dir).
- Per-phase timing in the pack output itself (it's UI-only, not in the XML/MD/plain text).
- Optimization of any phase (this release is measurement; optimizations land in v0.5 if data warrants).
- Pack history / Recents UI integration (option A from brainstorm question 2; deferred to a future release).
- Keyboard shortcuts (user explicitly excluded).
- Stacked-bar visual breakdown (Approach 2 from brainstorm; not picked).

---

## CHANGELOG entries (target text)

```markdown
## [Unreleased]

### Added
- Drag-and-drop folder selection — drop a folder anywhere on the app window to set it as the pack target. Files are resolved to their parent directory; multi-drop takes the first.
- Per-phase timing in `PackStats`: `walk_ms`, `process_ms`, `secret_scan_ms` (optional), `tokenize_ms` (optional), `emit_ms`. Surfaced as an inline breakdown row in the result panel; gives evidence for future perf decisions without doing premature optimization.

### Changed
- Pack screen auto-switches from GitHub URL mode to Folder mode when a folder is dropped onto the window.
```
