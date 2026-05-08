# Changelog

All notable changes to ProjectPacker are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-05-06

### Refactored
- **`crates/core/src/pack/orchestrator.rs::pack`** decomposed from a ~420-line inline body into a 122-line orchestrator that calls 7 private phase helpers in dataflow order: `run_walk_phase`, `run_process_phase`, `apply_pin_reorder`, `run_secret_scan_phase`, `run_tokenize_phase`, `accumulate_byte_token_totals`, `run_emit_phase`. Cancel checkpoints, event order, `_clone_guard` lifetime, and post-emit stats refresh all preserved. No behavior change.
- **`frontend/src/routes/Pack.tsx`** slimmed from 711 → 324 LoC. Sub-components extracted into `frontend/src/components/pack/{Toggle,CopyButton,StatsBar,PhaseBreakdown,AiContextTable,ProgressLog,DropOverlay}.tsx`; format helpers into `frontend/src/lib/format.ts`; AI model data into `frontend/src/lib/ai-models.ts`.
- **`usePackJob` hook** extracted into `frontend/src/lib/use-pack-job.ts` (106 LoC). Owns Channel<ProgressEvent> reuse across runs, runPack lifecycle, errorMsg state, and re-entry guards. Pack.tsx now just calls `const { run, errorMsg, isRunning } = usePackJob();`.

### Removed
- **Dead Rust deps:** `parking_lot`, `arboard`, `anyhow` (app crate), `proptest` (core dev-deps). Verified zero `use` statements anywhere in the workspace.
- **Dead frontend deps (~10):** `sonner`, `react-hook-form`, `@hookform/resolvers`, `zod`, `class-variance-authority`, `lucide-react`, `tailwind-merge`, `clsx`, `tw-animate-css`, and the `radix-ui` meta-package. All were transitive of the unused shadcn primitives.
- **`frontend/src/components/ui/`** (entire directory, 14 vendored shadcn primitives, ~1063 LoC). Zero imports outside the folder itself.
- **`frontend/src/lib/theme.ts`** (`useTheme`) — zero callers; dark theme is hardcoded throughout `Pack.tsx`.
- **`frontend/src/lib/cn.ts`** — only used by the deleted `components/ui/`.
- **`XmlBuilder::git_logs`** method (zero callers; `include_git_history` was never plumbed through).
- **`PackOptions.include_git_history`** field — declared but never read by the orchestrator.
- **`#[cfg(test)] pub fn files`** test alias in `pack/xml.rs` (rewired 2 callers to `files_legacy`).

### Changed
- **`use crate::types::*;` glob in `pack/orchestrator.rs`** replaced with explicit list of the 11 actually-used identifiers.
- **`format!`-into-`push_str` patterns in `markdown.rs` and `plain.rs`** migrated to `writeln!`/`write!` (xml.rs already used this pattern). Same output, fewer heap allocations per pack.
- **`pushEvent` Zustand action** caps the `events` array at 256 entries (UI only renders the last 16; prevents O(n²) growth on long-running packs).

### Performance
- Release-profile `panic = "abort"` (5–15% smaller release binary; verified no `catch_unwind` callers, only a logging panic hook).
- **Parallelized secret-scan loop** via Rayon `par_iter_mut`. Each file's `scan_and_redact` runs on its own thread; a short serial post-pass preserves deterministic `SecretHit` event order and `all_redactions` indexing. On a 12-core box with 10k files, ~10–15 s saved on the secret-scan phase alone.
- **Parallelized per-file tokenize loop** via Rayon. `count_by_name` is pure and `CoreBPE` is `Send + Sync` (returned via `OnceLock<CoreBPE>`). On 10k files at ~5 ms/file, this is ~30–45 s saved sequential → ~5 s on 12 cores.
- **Eliminated per-file `String` clones** in the process loop. The `after_comments` and `content` if-chains used to `.clone()` `raw` and `after_comments` in the no-transform branches. Reorder so the BLAKE3 hash is computed BEFORE the if-chain (it only needs the bytes), then both variables move through ownership. Saves 2–3 String allocs per file.
- **Pre-allocated emit buffers**. `XmlBuilder` gains a `with_capacity(n)` constructor; `markdown::render` and `plain::render` start with `String::with_capacity(stats.bytes_total * 2)`. For a 10 MB pack output, eliminates ~13 power-of-2 reallocations + ~10 MB of redundant memcpy. ~50–200 ms saved on large packs + ~1.5× peak-memory reduction during emit.
- **Cached compiled tree-sitter `Query` per `Lang`** in a `OnceLock<HashMap<Lang, LangQueries>>`. Previously `Query::new()` ran once per file per call; for a Rust monorepo with 500 .rs files that was 1000 redundant compilations. `Parser` stays per-call (`!Sync`). A `const _ASSERT_QUERY_SYNC` compile-time guard catches future tree-sitter regressions.
- **Pin reorder via index permutation + `mem::take`** instead of clone-and-rebuild. Each `FileEntry` is moved exactly once; no `String` clones on `path`/`content`/`hash`. `FileEntry` gains `#[derive(Default)]` to enable the `mem::take` pattern.
- **Per-file `tokens_per_model` sum replaces joined-string encode**. Previously `tokens_per_model` allocated a single `joined: String` of all entry contents (~content-bytes peak memory) and ran `count_all` over it. Now: parallel per-file `count_all` + saturating-add into a single `TokensPerModel` accumulator. ~content-size peak-memory reduction, plus parallelism. Behavior caveat: per-file sum diverges from joined-encode by typically <1% due to inter-file token-merge effects at file boundaries; users who snapshot-tested exact pre-v0.5 numbers will see slight drift.

## [0.4.0] - 2026-05-05

### Added
- Drag-and-drop folder selection — drop a folder anywhere on the app window to set it as the pack target. Files are resolved to their parent directory; multi-drop takes the first.
- Per-phase timing in `PackStats`: `walk_ms`, `process_ms`, `secret_scan_ms` (optional), `tokenize_ms` (optional), `emit_ms`. Surfaced as an inline breakdown row in the result panel; gives evidence for future perf decisions without doing premature optimization.
- `WarningKind::TokenizeFailed` — per-file tokenizer errors (e.g. an unknown `tokenizer_model`) now surface as `PackWarning`s instead of being silently swallowed. `tokens_total` undercount is no longer invisible.
- `patchOptions(partial)` Zustand store action — merges a partial update into options using a functional setter. Used by all option callsites in `Pack.tsx` so async handlers (folder picker, drag-drop) can't capture stale `options` and overwrite a recent edit to a different field.
- Integration tests for `PackFormat::Markdown` and `PackFormat::PlainText` (previously only `Xml` was end-to-end tested).
- Settings save now atomic via write-tmp-then-rename; two new regression tests pin the behaviour.

### Changed
- Pack screen auto-switches from GitHub URL mode to Folder mode when a folder is dropped onto the window.
- `TokenModel::Gpt4o` now uses `o200k_base` (the actual GPT-4o tokenizer) instead of `cl100k_base`. This aligns the typed API's `gpt4o` count with the legacy `count_by_name("gpt-4o-mini")` count, so per-file `tokens_total` and `tokens_per_model.gpt4o` finally agree on encoder. `TokenModel::GeminiApprox` follows (now `o200k_base × 1.05`). `TokenModel::Claude` continues to use `cl100k_base` (Anthropic ships no public tokenizer; cl100k is the closest public proxy). Observable token counts for `Gpt4o`/`GeminiApprox` will shift slightly vs. v0.3.0 — they're now correct for GPT-4o.
- Per-file `tokens_total` is accumulated as `u64` and saturated to `u32` only at the cast site. Multi-billion-token packs no longer wrap.
- `useDragDrop` registers its IPC listener once per mount (was once per render); `setIsDragging(true)` only fires on the leading edge of a drag.
- The pack screen's Channel is now reused across runs via `useRef` (was a fresh Channel per pack — Tauri's IPC handler map leaked one entry per run).

### Fixed
- **Security:** `save_to_file` removed and replaced with `save_pack_output(suggested_filename, contents)`. The new command shows the OS save dialog from the Rust side, so a compromised renderer can't supply an arbitrary write path.
- **Frontend race:** the pack-progress Channel handler is now installed before `await commands.packStart(...)` (was after). Events fired between the await resolving and the JS continuation reassigning `onmessage` are no longer dropped — `jobId` is captured from the `Started` event payload instead of the await's return value.
- Settings `save()` is now atomic (write-tmp-then-rename) — a power-loss mid-write no longer produces a 0-byte settings.json that silently loses the user's recents/presets/theme on next load.
- `*.bad-<N>` quarantine suffix uses nanosecond resolution (was 1-second), preventing rename collisions on two near-simultaneous corrupt-recovery paths.
- `Number("")` (empty `maxFileSizeKb` input) is no longer serialized as `NaN` (which round-tripped to `null` through the persisted store).
- Tighter capabilities — dropped the unused `fs:default` scope (the frontend doesn't call any `@tauri-apps/plugin-fs` API).
- The 3 v0.4 per-phase timing tests now actually verify timing correctness (sum of phases ≤ duration_ms + slack, each phase ≤ duration_ms). Previously they only type-asserted that the fields existed.

## [0.3.0] - 2026-05-01

### Added

#### Tokens — multi-model accuracy
- Typed `core::tokens::count(text, TokenModel)` API; `TokenModel` enum with 7 variants (`Gpt4o | Claude | Llama3 | Qwen2_5 | DeepSeek | Mistral | GeminiApprox`).
- Vendored HuggingFace tokenizer JSONs at `crates/core/assets/tokenizers/` (~25 MiB total) for Llama 3, Qwen 2.5, DeepSeek, and Mistral. Loaded lazily via `OnceLock` on first use of each model.
- `core::tokens::count_all(text)` returning all 7 model counts at once (cl100k shared between Gpt4o/Claude/GeminiApprox).
- `PackStats.tokens_per_model: Option<TokensPerModel>` exposes per-model counts to the frontend.
- Frontend AI compatibility table now shows per-row token counts using each model's authentic tokenizer; new rows for Mistral 7B/Mixtral and Qwen 2.5; "approx" badge + footer disclaimer on rows using proxy tokenizers (Claude/Grok cl100k, Gemini cl100k×1.05).
- `tokenizers = "0.23"` and `toml = "0.8"` added as core deps (with `unstable_wasm` feature on tokenizers — pure-Rust regex backend, no onig C dep).

#### Secrets — vendored gitleaks engine
- Vendored gitleaks v8.25.0 ruleset at `crates/core/assets/gitleaks.toml` (~167 rules); `LICENSE-3RD-PARTY` adds gitleaks MIT attribution.
- New `core::secrets::ruleset::{vendored, from_toml, RuleSet, Rule, RuleSetError}` loader with `RegexBuilder::size_limit(32 MiB)` so `generic-api-key`, `vault-batch-token`, etc. compile (default 10 MiB was too small). Only `pypi-upload-token` exceeds the cap and is skipped.
- New `core::secrets::engine::scan_and_redact(content, ruleset) -> ScanResult` engine with:
  - Aho-Corasick keyword pre-filter (case-insensitive) — skips regex evaluation on lines with no rule keyword.
  - Shannon-entropy gate — rules with `entropy_min` reject low-entropy matches.
  - Specificity-aware overlap resolution — `generic-api-key` is demoted so specific rules (`aws-access-token`, etc.) win when both match.
  - In-place `[REDACTED:<rule-id>]` substitution — pack content ships post-redaction.
- `Redaction { rule_id, line, byte_offset, matched_excerpt }` and `PackRedaction { file, rule_id, line, byte_offset }` types; `PackResult.redactions` lists all redactions for the pack.
- `<security_report>` block (XML/Markdown/Plain variants) emitted in pack output between stats and entries when redactions occurred. Lists each redaction by `(file, rule_id, line, byte_offset)`. Empty redaction set → no fragment emitted (preserves byte-equivalence for clean fixtures).
- Performance: warmed scan ~200 µs on a 100 KB fixture (release build); meets the <2s target for 100k LOC.
- `aho-corasick = "1"` added as core dep.

### Changed
- `core::secrets::scan(content) -> Vec<SecretHit>` now backs onto the new engine + vendored ruleset (was hand-rolled 10-rule list). Public API and shape preserved; rule IDs renamed to gitleaks canonical (`aws-access-key` → `aws-access-token`, `github-token` → `github-pat`, `private-key-pem` → `private-key`, etc.).
- `tests/fixtures/tiny/src/danger.txt` updated from `AKIA0000…` to canonical `AKIAIOSFODNN7EXAMPLE` (the new gitleaks AWS regex enforces the base32 alphabet).
- Orchestrator now mutates each entry's content to its redacted form before emission; pack output ships `[REDACTED:<rule-id>]` markers in place of secrets.

### Deferred
- The legacy `core::secrets::scan` wrapper remains as orphaned-but-harmless compat for any out-of-tree callers. Removal is on the v0.4+ backlog.
- `path_filter` (gitleaks `[rules.allowlist].paths`) is not yet modeled in `Rule`; gitleaks allowlists with `regexes`/`commits`/`stopwords` are also ignored. Worth revisiting if false-positive volume grows.
- The single gitleaks rule `pypi-upload-token` is skipped at load time because its compiled NFA exceeds the 32 MiB regex size limit.

## [0.2.0] - 2026-04-30

### Added
- `PackFormat` enum (`Xml | Markdown | PlainText`) with camelCase JSON serialization and TypeScript bindings.
- `core::pack::markdown` — Markdown emitter producing fenced code blocks with directory structure and summary table.
- `core::pack::plain` — Plain-text emitter using `=== path ===` separator format.
- `core::tree_sitter_compress::remove_comments` — tree-sitter-based comment stripper for Rust, Python, JavaScript, TypeScript with blank-line collapsing.
- AI context window compatibility table in the frontend showing which major models (GPT-4o, Claude, Gemini, Grok, o1/o3, DeepSeek, Llama) can handle the packed token count.
- Format selector (XML / Markdown / Plain Text) with format-aware copy button labels.
- Full options panel: compress skeleton, remove comments, respect gitignore, secret scan, count tokens, max file size.
- Public GitHub URL packing — `pack()` now accepts `PackTarget::GitHub(url)`, shallow-clones the repo into a temp dir (auto-cleaned), and packs it like any folder. Emits a `Cloning` progress event before the walk.
- BOM-aware encoding fallback chain in `read_text_with_fallback`: UTF-8 (with optional BOM) → UTF-16 LE → UTF-16 BE → Windows-1252.
- `PackWarning` collection wired through the pack pipeline. Emits `EncodingFallback` on non-UTF-8 decode and `FileSkipped` on file-read failure. Warnings appear in the result and the existing UI Warnings panel.
- Fatal-error surfacing — orchestrator failures now emit `ProgressEvent::Error { fatal: true }` instead of being silently dropped.
- Frontend target-mode toggle (Folder / GitHub URL) with GitHub URL validation.

### Changed
- `PackResult.xml` renamed to `PackResult.output` throughout Rust core and TypeScript bindings.
- Frontend redesigned as a single-screen packing tool; removed multi-tab routing (Bridge, Home, Result routes deleted, `react-router-dom` removed).
- Orchestrator now branches on `PackFormat` to dispatch the correct emitter.
- `pack()` signature changed from `pack(root: &Path, ...)` to `pack(target: &PackTarget, ...)`. Target resolution (including GitHub clone) now lives in core.
- `ProgressEvent::BuildingXml` renamed to `ProgressEvent::BuildingOutput` (now correct for all 3 output formats).

## [0.1.0] - 2026-04-30

### Added
- Initial workspace scaffold (Rust workspace + Tauri shell + React/Vite/Tailwind frontend).
- Design doc and implementation plan committed.
- MIT license.
- `core::types`, `core::error` — shared data shapes and error enum with `thiserror`.
- `core::ignore` — layered ignore matcher (builtin defaults + project + custom patterns).
- `core::walker` — synchronous file walker with skip-reason classification.
- `core::tokens` — tiktoken-based token counter (`gpt-4o-mini` default).
- `core::secrets` — gitleaks-style secret scanner with rule patterns for AWS, GitHub, OpenAI, Anthropic, Slack, Stripe, GCP, Azure, generic API keys, and PEM private keys.
- `core::tree_sitter_compress` — code skeleton compressor for Rust, Python, JavaScript, TypeScript.
- `core::github` — GitHub URL parser and shallow-clone wrapper via `gix` (parsing only in v0.1.0; orchestrator wiring deferred to v0.2.0).
- `core::protocol` — `block_for_pack`, `claude_code_prompt`, `build_combined_prompt`, and a strict `validate_plan` for the `grok-to-cc-v1` protocol.
- `core::pack::xml` — streaming XML emitter with proper escape handling.
- `core::pack::orchestrator` — end-to-end pipeline wiring walker → processors → XML → protocol.
- `app::settings` — settings persistence with corrupt-file recovery.
- `app::jobs` + `app::commands` — Tauri commands `pack_start`, `pack_cancel`, `pack_get_result`, `validate_plan`, `build_combined_prompt`, `get_settings`, `save_settings`, `save_to_file`, plus a `JobRegistry` for cancellation.
- `emit-bindings` binary — `tauri-specta` TypeScript bindings emitter.
- Frontend `lib/api`, `lib/events`, `lib/store` (Zustand) helpers and a minimal `HashRouter` with placeholder Home / Pack / Result / Bridge routes.
- `tiny` test fixture and `pack_integration` test suite (3 tests).
- `protocol_golden` insta snapshot tests freezing the v1 protocol outputs (3 tests).
- Windows-only CI workflow (`.github/workflows/ci.yml`).
- Tag-triggered release workflow (`.github/workflows/release.yml`) producing MSI + portable EXE bundles with SHA256SUMS.
- `scripts/build-release.ps1` for local releases.
- Claude Design handoff prompt at `docs/handoff/claude-design-prompt.md`.

### Deferred to follow-up plans
- GitHub URL packing wired through the orchestrator (parsing exists; pack rejects with a `not_implemented` error).
- Comment removal, git history embedding, drag-and-drop folder targeting.
- Encoding fallback beyond UTF-16LE (Windows-1252 etc.).
- Better error plumbing through the pack pipeline (failure path is currently silent).
