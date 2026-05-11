# ProjectPacker — Handoff

**Last updated:** 2026-05-11
**Project version:** 0.5.0 (origin/main `2282b2d`)
**Maintainer:** CBaileyDev
**Repo:** https://github.com/CBaileyDev/ProjectPacker

If you're picking this project up cold, read this top-to-bottom. It's the single source of truth for what the project is, how it's built, where things live, and what the current state is.

---

## 1. What it is

ProjectPacker is a **Windows Tauri 2 desktop app** (Rust core + React frontend) that packs a folder or a GitHub repo into a single self-describing **XML / Markdown / plain-text** file optimized for feeding to an AI assistant.

**The two-AI workflow it's built around:**
1. **Plan with Grok** — paste the pack into Grok, get back a structured plan in a strict format.
2. **Execute with Claude Code** — paste the plan + pack into Claude Code, which validates the plan against the embedded protocol and executes step-by-step.

The "protocol" is the `grok-to-cc-v1` format embedded in every pack (`docs/protocol/grok-to-cc-v1.md`). It's the contract between the two AIs.

**Hard constraints (fixed in `PLAN.md`):**
- **GUI-only.** No CLI mode, no `-0` flag, no headless binary, no MCP server mode.
- **Free, no API keys.** No Anthropic Files API integration, no paid SaaS, no telemetry.
- **Desktop-polish target.** Custom title bar, command palette, virtualized tree, native drag-drop are in scope; web/SaaS variants are not.

---

## 2. Current state

- **Tag history:** `v0.1.0`, `v0.4.0`, `v0.5.0` are all on GitHub.
- **Latest:** `2282b2d Phase 0 cleanup PR: 19 fixes from four-AI audit synthesis` (sitting on top of `7ba2349 v0.5.0 development snapshot`). History was rewritten on `main` recently — the linear 100+ commit history of v0.5.0 dev is collapsed into one snapshot commit, then the Phase 0 PR sits on top.
- **Tests:** 250 passing (38 + 203 + 6 + 3 across the four test binaries), 0 failures, 1 ignored (release-only perf test).
- **Build state:** `target/release/projectpacker-app.exe` is **47.3 MB**, built 2026-05-11 01:53 via `pnpm tauri build --no-bundle`.
- **Working tree:** clean except for `.claude/` (Claude Code config — gitignored locally) and `docs/superpowers/plans/2026-04-30-v02-single-screen-redesign.md` (an old plan that was never committed; predates current sessions).

---

## 3. Architecture

### High-level

```
ProjectPacker/
├─ crates/
│  ├─ core/      ← pure-Rust pack pipeline (no Tauri deps)
│  └─ app/       ← Tauri 2 desktop shell (IPC commands, settings, jobs)
├─ frontend/     ← React 19 + Tailwind v4 + Zustand single-screen UI
├─ docs/         ← specs, plans, protocol template, design handoff
├─ tests/        ← fixtures (tiny, medium, binary); e2e (empty placeholder)
├─ scripts/      ← build-release.ps1 (Windows release driver)
└─ .github/      ← CI (ci.yml) + Release workflow (release.yml)
```

### Two-crate Rust workspace

| Crate | Role | Key files |
|---|---|---|
| `projectpacker-core` | Pure-Rust pack pipeline. NO Tauri deps so it can be unit-tested without a desktop runtime. | `types.rs`, `error.rs`, `ignore.rs`, `walker.rs`, `detect.rs` (5-stage binary detection), `lang.rs`, `tokens.rs` (multi-model token counting), `secrets/{mod,engine,ruleset}.rs` (gitleaks engine), `tree_sitter_compress.rs`, `github.rs` (gix shallow clone), `protocol.rs` (Grok→CC template + plan validator), `pack/{orchestrator,xml,markdown,plain,pin,stats,security_report,mod}.rs` |
| `projectpacker-app` | Tauri 2 desktop shell. Owns IPC commands, job registry, persistent settings, binding generator. | `commands.rs` (the `#[tauri::command]` IPC surface), `jobs.rs` (`JobRegistry` for async job tracking), `settings.rs` (atomic write-tmp-then-rename to disk), `lib.rs` (Tauri builder + plugins), `main.rs` (entrypoint), `bin/emit-bindings.rs` (specta → TS bindings) |

### Frontend (`frontend/src/`)

| Dir | Contents |
|---|---|
| `routes/` | `Pack.tsx` (the single screen, 908 LoC; only file in this dir) |
| `components/pack/` | 12 sub-components: `Toggle`, `CopyButton`, `StatsBar`, `PhaseBreakdown`, `AiContextTable`, `ProgressLog`, `DropOverlay`, `GithubConnector`, `SaveButton`, `Settings`, `Skeleton`, `icons` |
| `lib/` | 12 modules: `store.ts` (Zustand), `api.ts` (typed re-exports), `events.ts` (IPC Channel helper), `persist-adapter.ts` (Tauri-Store-backed), `format.ts`, `ai-models.ts`, `motion.ts` (animations), `toast.ts`, `use-drag-drop.ts`, `use-github-token.ts`, `use-keyboard-shortcuts.ts`, `use-pack-job.ts` |
| `bindings/` | `index.ts` — **auto-generated** by `cargo run -p projectpacker-app --bin emit-bindings`. Gitignored locally but tracked in repo (legacy). Always regenerate after touching Rust types. |
| `styles/` | `globals.css` — Tailwind v4 `@theme` block |

### Pack pipeline phases (`crates/core/src/pack/orchestrator.rs`)

Function `pack(target, opts, tx, job_id, cancel, github_token) -> CoreResult<PackResult>`. Decomposed into 7 private phase helpers; each records its own `Instant`-based elapsed time. The phases run sequentially within `pack()`:

| Phase | Function | What |
|---|---|---|
| 1. Walk | `run_walk_phase` | `IgnoreMatcher` (3-tier: builtin / project gitignore / user `.repomixignore` + custom) → `walker::walk` → pin pre-pass (auto-includes `AGENTS.md`, `CLAUDE.md`, `.cursor/rules/`, `.claude/**`) |
| 2. Process | `run_process_phase` | Parallel (Rayon `par_iter`): read with encoding fallback (UTF-8 → UTF-16LE → UTF-16BE → Windows-1252), optional comment-removal (tree-sitter), optional skeleton compression, BLAKE3 hash |
| 3. Pin reorder | `apply_pin_reorder` | Index permutation + `mem::take` (no `String` clones) to push pinned files to the front |
| 4. Secret scan | `run_secret_scan_phase` | Parallel `par_iter_mut` running gitleaks `scan_and_redact` per file; serial post-pass for deterministic `SecretHit` event order + redaction aggregation |
| 5. Tokenize | `run_tokenize_phase` | Parallel per-file `count_by_name` + parallel per-file `count_all` summed via saturating-add into `TokensPerModel` |
| 6. (Accumulate byte/token totals — trivial helper) | `accumulate_byte_token_totals` | u64 accumulator, saturated to u32 at the cast site |
| 7. Emit | `run_emit_phase` | Pre-allocated `String::with_capacity(bytes_total * 2)`; routes to xml/markdown/plain renderer. The XML emitter writes the `<security_report>` block when redactions occurred. |

Cancel checkpoints: 3 in the `pack()` body + 1 per-file inside the `par_iter` closure. Event order is preserved deterministically via the serial post-pass after each parallel phase.

`tokens_per_model` is summed per-file rather than encoding a joined string — typically <1% drift from a true joined encode, but parallelizable and content-bytes-cheaper. Documented in CHANGELOG.

### Tauri IPC surface (`crates/app/src/commands.rs`)

The 8 `#[tauri::command]` functions exposed to the frontend (via `tauri-specta` typed bindings):

| Command | Purpose |
|---|---|
| `pack_start` | Kicks off a pack job. Returns `job_id` immediately; progress flows through a `Channel<ProgressEvent>` argument. |
| `pack_cancel` | Cancels by `job_id` via `CancellationToken`. |
| `pack_get_result` | One-shot retrieval of a completed pack result (DashMap remove). |
| `validate_plan` | Runs `protocol::validate_plan` on a markdown blob; returns structured `PlanValidation`. |
| `build_combined_prompt` | Generates the Grok→CC combined prompt for a plan. |
| `get_settings` / `save_settings` | Persists `Settings` (theme/recents/presets/templates) atomically. |
| `save_pack_output` | **Dialog-gated** file save — the OS save dialog runs in Rust, so a compromised renderer can't supply an arbitrary write path. (Removed `save_to_file` in v0.4 as a security fix.) |

---

## 4. Build & run — read this carefully

### ⚠️ Critical: use `pnpm tauri build`, NOT `cargo build --release` directly

**The gotcha:** `cargo build -p projectpacker-app --release` will produce a binary that compiles cleanly and IS in release mode, BUT Tauri's asset-embedding macro (`tauri::generate_context!`) does not reliably pick up the freshly built `frontend/dist/` files unless invoked through the Tauri CLI. The result is a 47 MB binary that boots, tries to load `http://localhost:1420`, and shows **"Hmmm… can't reach this page, localhost refused to connect"**.

**The fix:** always build the production binary via:

```bash
pnpm tauri build              # full build with MSI + NSIS installers
pnpm tauri build --no-bundle  # build only the .exe (faster, no installers)
```

This runs `beforeBuildCommand` (= `pnpm --filter projectpacker-frontend build`) → builds the frontend to `frontend/dist/` → compiles the Rust app with the right env so the embed succeeds.

For dev (hot reload, instant feedback):

```bash
pnpm tauri dev
```

This runs Vite dev server on `localhost:1420` AND launches the Tauri app pointing at it. Best for iterating.

### Other build commands

| Command | What it does |
|---|---|
| `pnpm install` | Install JS deps. Run after pulling. |
| `cargo run -p projectpacker-app --bin emit-bindings` | Regenerate `frontend/src/bindings/index.ts` from the Rust types via specta. Required after any change to `crates/core/src/types.rs`, `tokens.rs::TokenModel`, etc. The bindings file is gitignored but already tracked. |
| `pnpm --dir frontend build` | Build the production frontend bundle only (writes to `frontend/dist/`). |
| `pnpm --dir frontend typecheck` | TypeScript typecheck — zero errors expected. |
| `pnpm --dir frontend lint` | Biome lint. |
| `pnpm --dir frontend dev` | Vite dev server only (without the Tauri shell — for browser-only frontend dev). |
| `cargo test --workspace --tests` | Run all 250 tests (debug mode, ~25s). |
| `cargo test --workspace --tests --release -- --include-ignored` | Run all tests in release mode INCLUDING the perf test (slower compile, faster run, ~3-4 min total). |
| `cargo clippy --workspace --all-targets -- -D warnings` | Strict clippy. Currently passes clean. |
| `cargo build --workspace --release` | Compiles Rust release-mode but **DO NOT use this to produce the shippable .exe** — see warning above. |

### Output paths

- Release binary: `target/release/projectpacker-app.exe` (~47 MB)
- Installers: `target/release/bundle/msi/*.msi` and `target/release/bundle/nsis/*.exe` (only after `pnpm tauri build` with bundles)

---

## 5. Tests & verification

**250 tests** distributed across:

| Test binary | Count | What it covers |
|---|---|---|
| `projectpacker-app` lib tests | 38 | App-shell tests (`settings.rs` atomic save, `jobs.rs` JobRegistry, etc.) |
| `projectpacker-core` lib tests | 203 | All `#[cfg(test)] mod tests` inside core source files (types, ignore, walker, secrets engine, tokens, tree-sitter, protocol, pack phases) |
| `pack_integration.rs` | 6 | End-to-end against the `tests/fixtures/tiny` fixture — covers XML, Markdown, and Plain output paths + secret redaction |
| `protocol_golden.rs` | 3 | Insta snapshot tests for `grok-to-cc-v1` template stability |
| Ignored (release-only) | 1 | `secrets::engine::tests::perf_100k_under_two_seconds` — regex backtracking is debug-build slow; runs only with `--release --include-ignored`. Passes in ~2s release. |

**Always-green verification gate before push:**
```bash
cargo test --workspace --tests
cargo build --workspace --release    # NOTE: only verifies compile; for shippable binary use pnpm tauri build
cd frontend && pnpm typecheck
```

---

## 6. Key design decisions

### Vendored 25 MiB of HuggingFace tokenizer JSONs

`crates/core/assets/tokenizers/{llama-3,deepseek,qwen-2.5,mistral}.json` are embedded via `include_bytes!` so the app counts tokens for 7 models (GPT-4o, Claude, Gemini-approx, Llama 3, Qwen 2.5, DeepSeek, Mistral) with **zero network calls at runtime**. This is the deliberate dominant contributor to the 47 MB binary size; the alternative would be to fetch on first use and break the offline guarantee. Don't change this without a discussion.

- GPT-4o uses `o200k_base` (the actual tokenizer)
- Claude uses `cl100k_base` (Anthropic doesn't publish theirs; closest public proxy)
- Gemini-approx is `o200k_base × 1.05` rounded up (Google doesn't publish either)
- The 4 HF models use the vendored JSONs lazily — first use parses (~tens of ms), subsequent uses are zero-overhead.

`TokenModel::Gpt4o` was switched from `cl100k_base` → `o200k_base` in v0.4 so the typed API and the legacy `count_by_name("gpt-4o-mini")` agree on encoder.

### Vendored gitleaks ruleset

`crates/core/assets/gitleaks.toml` is the v8.25.0 gitleaks ruleset, ~167 rules. Loaded once via `OnceLock` at first scan. Three optimizations on top of raw gitleaks:

1. **Aho-Corasick keyword pre-filter** — skips regex evaluation on lines with no rule-keyword match.
2. **Shannon-entropy gate** — rules with `entropy_min` reject low-entropy noise.
3. **Specificity-aware overlap resolution** — `generic-api-key` is demoted so specific rules (`aws-access-token` etc.) win.

The single rule `pypi-upload-token` is dropped at load time because its compiled NFA exceeds the 32 MiB regex size limit. Documented in code + CHANGELOG.

The pack output ships post-redaction content (`[REDACTED:<rule-id>]` markers); the original secrets never appear in the pack output. The `<security_report>` block (XML) / equivalent section (MD/Plain) lists each redaction by `(file, rule_id, line, byte_offset)`.

### Default-ON v0.5 behaviors worth knowing

- `respect_gitignore: true` — `.gitignore` and `.git/info/exclude` are read.
- `secret_scan: true` — gitleaks redaction is on by default.
- `count_tokens: true` — all 7 token-count columns populate.
- `compress: false` — tree-sitter skeleton compression is OFF by default (it's a lossy transformation; users opt in).
- `remove_comments: false` — comment stripping is OFF by default (same reason).
- `format: PackFormat::Xml` — XML is the default (Claude Code and Grok both accept it natively).
- `xml_schema: XmlSchema::Cxml` — Anthropic-style `<documents>` schema is the default; legacy `<files>` is opt-in.

### Pinning (auto-include of instructional files)

`crates/core/src/pack/pin.rs` auto-pins these paths to the FRONT of the pack output (so AI sees them first):
- `AGENTS.md`
- `CLAUDE.md`
- All files under `.cursor/rules/`
- All files under `.claude/**`

User can override via `.repomixignore` (user-tier explicit excludes win over pinning).

### Atomic settings save

`crates/app/src/settings.rs::save` writes to `settings.json.tmp` then `std::fs::rename` over `settings.json`. Same-volume rename is atomic on both NTFS and POSIX, so a power-loss mid-write leaves either the previous good file or the new complete file — never a zero-byte corrupt file. Was a known data-loss bug pre-v0.4.

### GitHub PAT for private repos (new in Phase 0 cleanup PR)

`pack::pack()` gained a `github_token: Option<&str>` argument. The app reads the token from the OS keychain (via `use-github-token.ts` hook on the frontend, persisted via Tauri's `store` plugin) and forwards it to the core. **The token never crosses the JS↔Rust boundary except as an ephemeral argument to `pack_start`; it's not serialized in any persistent state on the Rust side.** None for folder targets or public repos.

---

## 7. Recent activity — Phase 0 cleanup PR

The current tip `2282b2d` ("Phase 0 cleanup PR: 19 fixes from four-AI audit synthesis") added substantial new features on top of the v0.5.0 snapshot:

| Area | What |
|---|---|
| **Frontend UI** | New components: `GithubConnector.tsx` (264 LoC), `SaveButton.tsx` (157), `Settings.tsx` (317), `Skeleton.tsx` (93), `icons.tsx` (74) |
| **Frontend lib** | New hooks/modules: `use-github-token.ts` (118), `use-keyboard-shortcuts.ts` (129), `motion.ts` (200), `toast.ts` (72), `api.ts` expanded to 87, `persist-adapter.ts` to 180 |
| **Rust core** | `pack::pack()` signature now takes `github_token: Option<&str>`; `tokens.rs` got a parallel `count_all_parallel` implementation that splits work across two Rayon-parallel branches (tiktoken group vs HF group) |
| **Rust app** | `commands.rs` expanded — new IPC surface for GitHub auth flow + save-output dialog |
| **Build/config** | New `pnpm-workspace.yaml` at root; root `package.json` now has `tauri` / `dev` / `build` / `bindings` scripts; `PLAN.md` (40 KB) added as the v0.2→v1.0 roadmap |

**The PR's "19 fixes from four-AI audit synthesis"** is the result of running 4 AI auditors over the v0.5.0 codebase and synthesizing the agreed-upon fixes. Details inside `PLAN.md`.

---

## 8. Known issues & quirks

| | |
|---|---|
| `cargo build --release` produces a "broken" binary | Use `pnpm tauri build --no-bundle` instead. Documented in §4. |
| `frontend/src/bindings/` is gitignored but tracked | Legacy state. Don't `git rm --cached` it; just regenerate when needed (`pnpm bindings`). |
| `pypi-upload-token` gitleaks rule is skipped | Its compiled regex exceeds 32 MiB. Logged at scan time. Documented. |
| `cargo fmt --check` shows 66 pre-existing diffs | Codebase has never been run through `cargo fmt --all`. Not introduced recently. Can be fixed in a future cleanup pass. |
| `pnpm lint` has 2 formatter nits in `Pack.tsx:245` and `use-drag-drop.ts:39` | Line-wrap preferences. Pre-existing. Run `pnpm --dir frontend format` if you want them gone. |
| `pnpm test` (vitest) → "no test files" | Vitest is configured but the frontend has no `*.test.{ts,tsx}` files. Coverage gap, not a regression. |
| `tests/e2e/` is empty | Aspirational scaffolding. Either populate or remove. |
| `tests/fixtures/{medium,binary}/` are empty directories | Same. |
| `cargo doc` warns: `unresolved link to tokenizers::Tokenizer::encode` | Pre-existing doc-comment in `tokens.rs`. |
| `cargo fmt` on non-ASCII commit messages on Windows | Be careful with `×` / em-dashes in HEREDOC commit messages on PowerShell — they can mangle. Use `x` in commit bodies. |
| Pre-existing untracked file: `docs/superpowers/plans/2026-04-30-v02-single-screen-redesign.md` | Predates current sessions. Untouched. |
| The `tauri-plugin-fs` Rust crate is still loaded in `lib.rs:17` | The `fs:default` capability scope was dropped, but the plugin itself is needed because `use-drag-drop.ts` calls `@tauri-apps/plugin-fs::stat`. Don't unload the plugin. |
| `setOptions` Zustand action exists but has zero callers | Replaced by `patchOptions` in v0.4. Kept defensively — verification was ambiguous. Safe to delete in a future pass after a confirming grep. |

---

## 9. CI / release

- `.github/workflows/ci.yml` — runs on every push to `main` / PR. Build + test + typecheck on `windows-latest`.
- `.github/workflows/release.yml` — triggered by `v*.*.*` tag push. Builds MSI + NSIS installers, attaches SHA256SUMS, drafts a GitHub Release. **Currently Windows-only** (matrix would need adding for Mac/Linux).
- `scripts/build-release.ps1` — local Windows release driver. Mirrors what CI does.

To cut a release:
```bash
# 1. Bump versions in lockstep
#    - Cargo.toml (workspace.package.version)
#    - crates/app/tauri.conf.json
#    - frontend/package.json
# 2. Promote CHANGELOG [Unreleased] to [X.Y.Z] - YYYY-MM-DD
# 3. Verify locally:
cargo test --workspace --tests
pnpm tauri build --no-bundle
# 4. Commit + push + tag:
git add Cargo.toml crates/app/tauri.conf.json frontend/package.json CHANGELOG.md
git commit -m "chore(release): bump to X.Y.Z and finalize CHANGELOG"
git push origin main
git tag -a vX.Y.Z -m "vX.Y.Z — <summary>"
git push origin vX.Y.Z
# 5. release.yml fires, builds installers, drafts a GitHub Release.
```

---

## 10. Tech stack reference

**Rust deps worth knowing:**
- `rayon` — data-parallel `par_iter_mut`. Used in all 5 pack phases that benefit.
- `BLAKE3` (`blake3 = { features = ["rayon", "mmap"] }`) — content hashing; mmap'd for files > 256 KB.
- `tree-sitter = "0.25"` + 4 grammar crates (rust, python, javascript, typescript). Queries cached per-Lang in `OnceLock<HashMap>`. Parser stays per-call (not Sync).
- `tiktoken-rs = "0.6"` — OpenAI tokenizers (`o200k_base`, `cl100k_base`).
- `tokenizers` (HuggingFace) — for the 4 vendored HF tokenizer JSONs.
- `gix = "0.66"` (default-features off + minimal features) — shallow clone for GitHub targets.
- `encoding_rs` — UTF-16 LE/BE + Windows-1252 fallback decoders.
- `regex` (with 32 MiB size limit), `aho-corasick` — secrets engine.
- `ignore` — gitignore matcher.
- `quick-xml` — XML emission with proper escape handling.
- `specta` (`=2.0.0-rc.22`) + `specta-typescript` (`=0.0.9`) — typed TS binding generation. Versions pinned hard because of camelCase rename quirks.
- `tauri = "2"` + 8 plugins: dialog, fs, clipboard-manager, shell, store, log, window-state, single-instance.

**Frontend stack:**
- React 19, TypeScript 5.6, Vite 6
- Tailwind v4 (with `@theme` block in `globals.css`)
- Zustand 5 (with `persist` middleware backed by Tauri's `store` plugin)
- Biome (lint + format)
- Vitest (configured, no tests written)

**Release profile (`Cargo.toml` workspace):**
```toml
[profile.release]
strip = true
lto = "thin"
codegen-units = 1
panic = "abort"
```
Verified no `catch_unwind` callers anywhere → `panic = "abort"` is safe.

---

## 11. Deferred / on the v0.6+ horizon

Pulled from `PLAN.md` and from the v0.5.0 deferred list:

- **Cross-platform builds.** Mac + Linux via GitHub Actions matrix. Tauri 2 supports both natively; only `release.yml` and `tauri.conf.json` `bundle.targets` need changes (no source changes required — verified portable).
- **`WalkDir` → `ignore::WalkParallel`.** 3–5× walker speedup on multi-core. Requires rewriting `IgnoreMatcher` to feed patterns into `WalkBuilder` directly rather than post-filtering.
- **Eliminate double file-read in walker.** Today the walker does an 8 KB binary-detect read, then the processor re-opens for full content. Could merge into one read.
- **Feature-gate vendored HF tokenizers.** Drops 25 MiB from binary if user only wants GPT/Claude counts. Trade-off: breaks the offline guarantee.
- **Command palette** — Cmd/Ctrl+K to access any action.
- **Virtualized file tree** — for picking subsets of large repos.
- **Pack history UI** — using the per-phase timing data we capture but don't surface beyond the inline row.
- **Tighter CSP** — drop `'unsafe-inline'` from `style-src` (was needed for the now-deleted shadcn `progress.tsx`).
- **Add at least one vitest test** so `pnpm test` stops saying "no test files found".

---

## 12. Useful command cheatsheet

```bash
# === everyday dev ===
pnpm tauri dev                                # hot-reload dev shell
cargo test --workspace --tests                # all 250 tests, ~25s
pnpm --dir frontend typecheck                 # TS typecheck
pnpm --dir frontend lint                      # Biome lint

# === regenerate bindings after Rust type changes ===
cargo run -p projectpacker-app --bin emit-bindings
# or:
pnpm bindings

# === build the shippable .exe (DO THIS, NOT cargo build --release) ===
pnpm tauri build --no-bundle                  # → target/release/projectpacker-app.exe
pnpm tauri build                              # also bundles MSI + NSIS installers

# === full verification gate before push ===
cargo test --workspace --tests
cargo build --workspace --release             # compile-only sanity check
pnpm --dir frontend typecheck

# === release-mode test pass (includes perf test) ===
cargo test --workspace --tests --release -- --include-ignored

# === strict lint sweep ===
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 13. People & history

- **Maintainer:** CBaileyDev (solo developer; works directly on `main`, no PR workflow needed for personal use)
- **AI co-developers:** Claude (Opus 4.7) has done significant chunks of v0.3/v0.4/v0.5/Phase 0 work via subagent orchestration. Commits show `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>` in the bodies.
- **Workflow norms:**
  - Direct commits to `main`. No PRs (for solo work).
  - Conventional Commits style: `feat(scope): ...`, `chore(scope): ...`, `fix(scope): ...`, etc.
  - Multi-line commit bodies via HEREDOC.
  - Tags follow SemVer.
  - History was rewritten once (v0.5.0 dev squashed into one snapshot commit). Reflog has the full history for 90 days.

---

## 14. If you're future-Claude reading this

- The `superpowers:*` skill family is in active use (brainstorming, writing-plans, executing-plans, subagent-driven-development).
- `docs/superpowers/specs/` and `docs/superpowers/plans/` are the canonical home for new specs and plans.
- The user's preference is **autonomous overnight delegation when explicitly authorized.** Memory file `workflow_preferences.md` in `~/.claude/projects/.../memory/` captures this.
- The user works directly on `main`; do not create feature branches unless asked.
- Match the existing commit-message convention exactly (including the `Co-Authored-By` footer for AI-assisted work).
- The bindings file `frontend/src/bindings/index.ts` is gitignored but tracked — when you `git add` it expect a "paths ignored" warning. Just commit; the staging works.
- The `pack.xml` at the repo root is a leftover from manual testing, gitignored — not something to track.

Welcome aboard.
