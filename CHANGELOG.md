# Changelog

All notable changes to ProjectPacker are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
