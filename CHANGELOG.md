# Changelog

All notable changes to ProjectPacker are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
