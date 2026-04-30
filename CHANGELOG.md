# Changelog

All notable changes to ProjectPacker are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
