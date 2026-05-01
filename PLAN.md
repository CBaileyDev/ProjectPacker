# ProjectPacker — Phased Implementation Plan

**Document status:** active roadmap from v0.2 → v1.0
**Owner:** CBaileyDev
**Last updated:** 2026-04-30

---

## 0. Constraints & Decisions

These are fixed for the entire roadmap and shape every phase below.

| Constraint | Implication |
|---|---|
| **GUI-only**, no CLI for end users | No NUL stdin, no `-0` flag, no MCP server mode, no headless binary |
| **Free app**, no API keys | No Anthropic Files API integration, no paid SaaS, no telemetry behind a key |
| **Desktop-app polish target** | Custom title-bar, command palette, virtualized tree, native drag-drop are in scope |
| **Two-AI workflow stays core** | Plan with Grok → Execute with Claude Code; protocol layer is preserved and enhanced, not replaced |

### Items removed from the original list (and why)

- **MCP server mode** — requires being launched as a subprocess by Claude Code (headless). Violates GUI-only.
- **Anthropic Files API** (`--upload-to-files-api`) — requires storing an API key. Violates no-API-keys.
- **NUL-separated stdin (`-0`)** — CLI feature. Violates GUI-only.
- **All CLI-shaped affordances** — `projectpacker --map`, `git ls-files -z | projectpacker -0`, etc. Reframed as in-app toggles where they apply (e.g. "Map Mode" is a UI option, not a flag).

### Items kept but rescoped against original estimates

| Item | Original estimate | Realistic estimate | Why the gap |
|---|---|---|---|
| Custom title-bar | ~80 LOC | ~150-200 LOC | Win11 snap layouts, focus state, multi-DPI hit-targets, drag region, double-click maximize |
| Personalized PageRank | ~40 LOC | ~40 LOC algo + 800-1500 LOC symbol-graph extraction across 17 languages | The algorithm is small; per-language defs/refs/imports extraction is the real work |
| 13 new tree-sitter grammars | dependency add | dep add + 1 outline query (`signatures.scm`) per language | Grammars don't ship with outline queries; we author/maintain them |
| Vendored gitleaks rules | dep add | dep add + ~300 LOC TOML loader, entropy gate, keyword pre-filter | Gitleaks itself is Go; we re-implement the matching engine |

---

## 1. Architecture Targets at v1.0

This is what the codebase looks like when everything in this plan ships:

```
ProjectPacker/
├── crates/
│   ├── core/                      # Pure-Rust packing engine
│   │   ├── walker/                # ignore-walk + parallel pipeline
│   │   ├── ignore/                # 3-tier stacking, .repomixignore compat
│   │   ├── detect/                # binary detection (5-layer)
│   │   ├── hash/                  # BLAKE3 (replaces sha2)
│   │   ├── secrets/               # vendored gitleaks rules + redactor
│   │   ├── tokens/                # tiktoken-rs + HF tokenizers
│   │   ├── compress/              # tree-sitter 4-mode (none/outline/+keylines/skeleton)
│   │   ├── map/                   # PageRank symbol-graph repo-map
│   │   ├── cache/                 # content-addressed + zstd
│   │   ├── pack/                  # xml/markdown/plain emitters + orchestrator
│   │   ├── protocol/              # grok-to-cc-v1 + SEARCH/REPLACE envelope
│   │   ├── templates/             # Tera prompt gallery
│   │   └── grammars/              # 17 tree-sitter languages + signatures.scm
│   └── app/                       # Tauri shell
│       ├── commands.rs            # Tauri commands (typed, ipc::Channel)
│       ├── jobs.rs                # CancellationToken-based job registry
│       └── settings.rs            # via tauri-plugin-store
├── frontend/                      # React 19 + Vite + TS + Tailwind 4
│   └── src/
│       ├── components/ui/         # shadcn primitives
│       ├── components/tree/       # virtualized file tree
│       ├── components/preview/    # shiki-highlighted XML
│       ├── components/palette/    # cmdk command palette
│       ├── components/titlebar/   # custom min/max/close
│       ├── lib/
│       │   ├── api.ts             # specta-typed Tauri commands
│       │   ├── store.ts           # Zustand + persist
│       │   ├── theme.ts           # ~20 LOC theme hook
│       │   └── cn.ts              # clsx + tailwind-merge
│       └── routes/Pack.tsx
└── docs/
    ├── superpowers/specs/
    └── superpowers/plans/
```

---

## 2. Phase Overview

Seven phases, each shippable as a versioned release. Each phase has user-visible value on its own.

| Phase | Version | Theme | Duration estimate |
|---|---|---|---|
| 1 | v0.2.0 | Foundation cleanup + plumbing | ~1 week |
| 2 | v0.3.0 | Token accuracy + secret hardening | ~1 week |
| 3 | v0.4.0 | Compression + caching at scale | ~2 weeks |
| 4 | v0.5.0 | UX polish (tree, palette, drag-drop) | ~2 weeks |
| 5 | v0.6.0 | Map Mode (top 4 languages) | ~1.5 weeks |
| 6 | v0.7.0 | Map Mode (all 17) + plan envelope + templates | ~1.5 weeks |
| 7 | v1.0.0 | Title-bar + final polish + 1.0 release | ~1 week |

**Total estimate:** ~10 weeks of focused work for one developer.

---

## Phase 1 — Foundation Cleanup (v0.2.0)

**Theme:** invisible plumbing swaps that unblock everything below. No user-visible new features, but every later phase depends on this.

### Scope

#### Rust core
- **Replace `sha2` with `blake3` 1.8** (`["rayon", "mmap"]` features)
  - Files: `crates/core/Cargo.toml`, hash callsites in `pack/orchestrator.rs`
  - Use `Hasher::update_mmap_rayon` for files >256KB
- **`tokio-util` 0.7.18** (`CancellationToken` + `TaskTracker`)
  - Replace ad-hoc cancellation in `app/jobs.rs` with formal `CancellationToken`
  - Pack pipeline checks token at every stage boundary (walk → process → emit)
- **`tracing-tree` 0.4.1** (dev-only feature)
  - Indented span tree in `RUST_LOG=debug` runs
- **3-tier ignore stacking** in `core/ignore`
  - Stack: builtin defaults → project `.gitignore`/`.git/info/exclude` → user globs
  - Read `.repomixignore` for migration compat (treat as user layer)
  - File: `crates/core/src/ignore.rs`
- **Layered binary detection** in `core/detect/`
  - Order: extension allow-list → extension deny-list → NUL-byte sniff → `infer` magic-number → `file-format` fallback
  - Replace dead `content_inspector` references; add 30-line BOM+NUL sniffer
  - Deps: `infer = "0.19"`, `file-format = "0.29"`
- **Stats block at top of every pack**
  - File count, included tokens, excluded tokens (compressed/redacted), cache-hit count, language breakdown
  - ~150 tokens; primes the LLM with situational awareness
- **Auto-pin instructional files** (always emitted first when present)
  - Pin list: `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.cursorrules`, `.cursor/rules/*.mdc`, `.github/copilot-instructions.md`, `.aider.conf.yml`, `.windsurfrules`, `.claude/**`, `.context/index.md`, `README.md`
  - Implementation: pre-scan pass before main walk, force these to render at top regardless of ignore state (unless user explicitly excludes)
- **Tail-priority ordering** in XML emitter
  - Stats block + auto-pinned files at top; bulk reference content at top; most-relevant files render closest to the implicit user-turn at bottom
  - Configurable per format (XML uses tail-priority by default; Markdown stays alphabetical for diffability)
- **Anthropic `<documents>` cxml schema refinement** for XML format
  - Switch from current `<file>` shape to Anthropic's `<document index="N"><source>path</source><document_content>...</document_content></document>` schema
  - Measurably better extraction on Claude per Anthropic's published guidance
  - Keep the existing schema available behind a setting for back-compat

#### Tauri app
- **Migrate `app.emit()` → `ipc::Channel<T>`** for `ProgressEvent`
  - Typed, ordered, command-scoped streaming
  - tauri-specta derives the TS type for free
  - File: `crates/app/src/commands.rs` — `pack_start` returns `ipc::Channel<ProgressEvent>`
- **`tauri-plugin-store`** + persist adapter (~30 LOC) for Zustand
  - Presets/config to OS app-config-dir
  - Survives reinstall (browser localStorage doesn't)
- **`tauri-plugin-window-state`** — restore size/pos/maximized
- **`tauri-plugin-single-instance`** — prevents two windows racing on the same output dir
- **`tauri-plugin-log`** — replaces `tracing-appender`
  - Bridges Rust `tracing` to disk + frontend `console`
  - Removes the hand-rolled rotation in `app/lib.rs`
  - Frontend can subscribe to log stream for in-app log viewer (later phase)

#### Frontend foundation (greenfield additions, no migration)
- **`clsx`** + **`tailwind-merge` 3.x** + **`cva`** — `cn()` utility (`lib/cn.ts`, ~10 LOC)
  - Tailwind-merge v3 specifically (required for Tailwind 4 utility renames)
- **`lucide-react`** (named imports only — per-icon tree-shake)
- **shadcn/ui primitives** — install via shadcn CLI:
  - `Button`, `Input`, `Label`, `Card`, `Dialog`, `Sheet`, `Tooltip`, `ScrollArea`, `Tabs`, `Select`, `Switch`, `Checkbox`, `Form`, `Toast` (sonner adapter), `Popover`, `Separator`, `Badge`, `Slider`, `Progress`
- **`react-error-boundary`** — wrap app shell + each route
- **Hand-rolled theme hook** (~20 LOC, `lib/theme.ts`)
  - Tailwind 4 `@theme` + `dark` class on `<html>` + `matchMedia('(prefers-color-scheme: dark)')` listener
  - Replaces `next-themes` (Next-coupled, irrelevant for Tauri)
- **Hand-rolled persist adapter** (~30 LOC, `lib/persist-adapter.ts`)
  - tauri-plugin-store ↔ Zustand bridge
- **`@biomejs/biome`** — replaces ESLint + Prettier
  - One Rust binary, one config (`biome.json`)
  - Add `pnpm lint`, `pnpm format` scripts

### Files affected (Phase 1)

```
crates/core/Cargo.toml                       # +blake3, +tokio-util, +tracing-tree, +infer, +file-format; -sha2, -content_inspector
crates/core/src/ignore.rs                    # 3-tier stack + .repomixignore compat
crates/core/src/detect.rs                    # NEW - 5-layer binary detection
crates/core/src/pack/orchestrator.rs         # blake3 swaps, cancellation token, stats block, auto-pin pre-pass
crates/core/src/pack/xml.rs                  # cxml schema, tail-priority ordering
crates/core/src/pack/stats.rs                # NEW - stats block emitter
crates/core/src/pack/pin.rs                  # NEW - instructional-file pinner
crates/app/Cargo.toml                        # +tauri-plugin-* (4 plugins); -tracing-appender
crates/app/src/commands.rs                   # ipc::Channel migration
crates/app/src/jobs.rs                       # CancellationToken
crates/app/src/lib.rs                        # plugin registrations
frontend/package.json                        # +clsx, +tailwind-merge@3, +cva, +lucide-react, +sonner, +react-error-boundary, +@biomejs/biome
frontend/components.json                     # NEW - shadcn config
frontend/src/components/ui/                  # NEW - shadcn primitives
frontend/src/lib/cn.ts                       # NEW
frontend/src/lib/theme.ts                    # NEW
frontend/src/lib/persist-adapter.ts          # NEW
frontend/biome.json                          # NEW
```

### Acceptance criteria

- [ ] Existing `pack_integration` tests pass unchanged.
- [ ] `pack_start` over `ipc::Channel` delivers `ProgressEvent`s ordered, no drops vs current `app.emit()`.
- [ ] Cancelling a pack mid-walk returns within 200ms.
- [ ] Re-launching app restores window size/position.
- [ ] Closing app, launching second instance focuses first.
- [ ] Settings file written via `tauri-plugin-store` survives a clean reinstall (test on local Windows VM).
- [ ] `pnpm lint` runs Biome and passes on existing TS.
- [ ] BLAKE3 hashing of the `tiny` test fixture matches a known-good vector.
- [ ] cxml-formatted XML output validates against a small XSD or hand-checked golden.

### Risks & mitigations

- **Tauri plugin compat with v2 API churn:** All four plugins are official and ABI-stable on Tauri 2. Pin exact versions in `Cargo.toml`.
- **`tauri-plugin-log` rotation differs from `tracing-appender`:** Document new log path in `docs/`, add Settings → "Open Log Folder".
- **shadcn/ui copies source — vendor lock:** This is by design (you own the components). Set up a `pnpm shadcn-update` script for tracking upstream churn.

---

## Phase 2 — Token Accuracy + Secret Hardening (v0.3.0)

**Theme:** two big honest-quality improvements that are mostly invisible-but-felt.

### Scope

#### Tokens — `tokenizers` crate + vendored JSONs
- **Add `tokenizers = "0.23"` with `["unstable_wasm"]` feature**
  - Despite the name, this enables the pure-Rust `regex` backend instead of `onig` (C dep)
  - Eliminates the onig Tauri Windows cross-compile pain point
- **Vendor 4 tokenizer JSONs** in `crates/core/grammars/tokenizers/` (or `crates/core/assets/tokenizers/`):
  - `llama-3.json` (~9MB)
  - `qwen-2.5.json` (~7MB)
  - `deepseek.json` (~6MB)
  - `mistral.json` (~2MB)
  - Total cold weight: ~24MB; load lazily on first request per model
  - Embed via `include_bytes!` so they ship in the installer (no first-run download)
- **Keep `tiktoken-rs`** for OpenAI / Claude (cl100k-compatible)
- **`gemini` is approximated** with a known cl100k delta + disclaimer in UI
- **Token-counter API** in `core/tokens.rs`:
  - `count(text: &str, model: TokenModel) -> usize`
  - `TokenModel`: `Gpt4o | Claude | Llama3 | Qwen2_5 | DeepSeek | Mistral | GeminiApprox`
- **UI**: AI context window compatibility table (already exists per CHANGELOG) gets accurate per-model counts; add per-model badge

#### Secrets — vendored gitleaks ruleset
- **Vendor `gitleaks.toml`** at `crates/core/assets/gitleaks.toml`
  - Source: gitleaks/gitleaks repo, MIT license. Add LICENSE attribution to repo `LICENSE-3RD-PARTY` file.
  - ~167 rules (AWS/GCP/Azure/Slack/Stripe/PEM/JWT/private keys/etc.)
- **Replace current rule-list in `core/secrets.rs`** with TOML-loaded ruleset
  - Parser: `toml = "0.8"` (likely already transitive)
  - Each rule: `id`, `description`, `regex`, `keywords[]`, `entropy_min`, `path_filter`
- **Engine optimizations:**
  - **Keyword pre-filter** — skip regex eval if no rule keyword present in chunk (5-10× speedup)
  - **Entropy gate** — for high-FP rules (generic-api-key, etc.), reject matches below `entropy_min` Shannon entropy
  - Batch regex compilation up front; reuse `RegexSet` per scan
- **In-place redaction** (replaces current "skip whole file" behavior)
  - Replace match span with `[REDACTED:rule-id]`
  - File still ships, signal preserved (Secretlint's flaw was nuking the whole file)
- **`<security_report>` block** appended to pack
  - Lists `(rule_id, file, line, redacted_at_offset)` per redaction
  - Lets the LLM see "I redacted these 3 things" without seeing the secrets

### Files affected (Phase 2)

```
crates/core/Cargo.toml                       # +tokenizers, +toml
crates/core/assets/gitleaks.toml             # NEW (vendored)
crates/core/assets/tokenizers/*.json         # NEW (4 files, ~24MB)
crates/core/src/tokens.rs                    # multi-model API
crates/core/src/secrets.rs                   # full rewrite — TOML loader, keyword pre-filter, entropy gate, redactor
crates/core/src/pack/security_report.rs      # NEW
crates/core/src/pack/orchestrator.rs         # wire redaction + security report
LICENSE-3RD-PARTY                            # NEW - gitleaks attribution
frontend/src/routes/Pack.tsx                 # per-model badge
```

### Acceptance criteria

- [ ] Pack a fixture containing a fake AWS access key — pack output contains `[REDACTED:aws-access-token]`, `<security_report>` lists the rule, no plaintext key in output.
- [ ] Token count for a Llama-3 fixture matches `transformers` Python reference within ±2 tokens on 10 sample files.
- [ ] Secret scan of 100k-LOC Linux kernel snippet completes in <2s on a mid-tier laptop (target: 5-10× faster than naive regex-only).
- [ ] Installer size increases by ≤30MB due to vendored tokenizers; documented in CHANGELOG.

### Risks & mitigations

- **Gitleaks false-positive rate:** Inherent to keyword + regex matching. Mitigate by exposing per-rule disable in Settings, defaulting noisy rules (`generic-api-key`) to entropy-gated mode.
- **Tokenizer file size in installer:** ~24MB cold. Lazy-load to avoid ~60ms startup hit. Document tradeoff in README.
- **Vendored ruleset goes stale:** Add a CI workflow that nightly-pulls upstream gitleaks.toml and opens a PR if changed. Manual review before merging.

---

## Phase 3 — Compression + Caching at Scale (v0.4.0)

**Theme:** the performance-and-tokens phase. Big repos become viable.

### Scope

#### Tree-sitter — language coverage + 4 modes
- **Bump `tree-sitter` 0.25 → 0.26**
  - Required ABI for the new grammars
- **Add 13 new grammars:**
  - `tree-sitter-go`, `-java`, `-c`, `-cpp`, `-c-sharp`, `-ruby`, `-php`, `-html`, `-css`, `-json`, `-md` (0.5), `-bash`, `-yaml`
  - Cargo dep on each; lazy-init parsers
  - Total: 17 languages (4 existing + 13 new)
- **Author `signatures.scm` per language** in `crates/core/grammars/queries/{lang}/signatures.scm`
  - Captures: `@function.name`, `@class.name`, `@method.name`, `@type.name`, `@import`, `@export`
  - Adapt from Aider's `tags.scm` queries (Apache 2.0, attribute in `LICENSE-3RD-PARTY`)
- **4-mode compression** in `core/compress/`:
  - `None` — verbatim (current default)
  - `Outline` — signatures + bodies elided to `// ...` (~70% token reduction)
  - `OutlineKeylines` — signatures + first 1-2 body lines retained for context
  - `Skeleton` — signatures only, tree structure preserved (~95% reduction)
- **Per-file budget**: switch mode automatically when verbatim exceeds `--max-tokens-per-file`
- **`text-splitter = "0.30"` with `["code", "tokenizers"]`** for over-budget files
  - Splits at AST boundaries (function/class), not random byte offsets
  - Used when even Outline mode exceeds the per-file budget

#### Caching
- **Content-addressed pack cache**
  - Cache key: `BLAKE3({canonical_path, file_size, mtime_ns, transform_version, ruleset_hash, grammar_version})`
  - Storage: OS app-cache-dir (Windows: `%LOCALAPPDATA%\ProjectPacker\cache\`)
  - Each entry: `{key}.zst` containing serialized `ProcessedFile` (post-compression, post-redaction)
- **`zstd = "0.13"`** for cache compression (5-10× smaller, near-memcpy decompress)
- **Cache invalidation:**
  - `transform_version` bumped on any compression-engine change
  - `ruleset_hash` is BLAKE3 of vendored `gitleaks.toml` — auto-invalidates when rules update
  - `grammar_version` bumped per tree-sitter grammar update
- **BLAKE3 + size pre-filter dedup**
  - Dedup pass after walk: identical files → one inline body in pack + N path refs in manifest
  - Saves substantial tokens in monorepos with vendored copies (`node_modules` mirrors, vendored crates, etc.)
- **Settings → "Clear Cache"** UI button

### Files affected (Phase 3)

```
crates/core/Cargo.toml                       # +tree-sitter-* (13), +text-splitter, +zstd
crates/core/src/compress/mod.rs              # NEW - mode selector
crates/core/src/compress/outline.rs          # NEW
crates/core/src/compress/skeleton.rs         # NEW
crates/core/src/compress/keylines.rs         # NEW
crates/core/src/grammars/mod.rs              # NEW - lazy parser pool
crates/core/src/grammars/queries/*/signatures.scm  # 17 query files
crates/core/src/cache.rs                     # NEW - content-addressed cache
crates/core/src/dedup.rs                     # NEW - BLAKE3 + size pre-filter
crates/core/src/pack/orchestrator.rs         # cache + dedup integration
crates/app/src/commands.rs                   # +clear_cache command
frontend/src/routes/Pack.tsx                 # mode selector, "Clear Cache" button
LICENSE-3RD-PARTY                            # +Aider tags.scm attribution
```

### Acceptance criteria

- [ ] Re-pack of a 100k-file repo (cold cache vs warm cache): warm completes in <500ms.
- [ ] Outline mode on a 1000-LOC Rust file produces ≤30% of original token count.
- [ ] Skeleton mode on the same file produces ≤10% of original token count.
- [ ] Cache directory zstd compression achieves ≥5× ratio on text-heavy entries.
- [ ] Bumping a single grammar version invalidates only its language entries.
- [ ] Dedup test: 10 copies of the same file → 1 inline body + 10 path refs.
- [ ] All 17 grammars parse without panic on a stress fixture (curated 100-file mix).

### Risks & mitigations

- **Grammar ABI churn between tree-sitter releases:** Pin exact versions; document upgrade procedure. Add a `grammar-smoke` test that parses one fixture per language.
- **`signatures.scm` query bugs:** Each query gets a golden test (insta snapshot of extracted symbols).
- **Cache directory growth unbounded:** Add LRU eviction at 2GB total or 30 days unused, whichever first. Configurable in Settings.
- **text-splitter splits inside string literals on JSON/YAML:** Mark these as "no-split" grammars — they get verbatim or hard-cut, not AST-split.

---

## Phase 4 — UX Polish (v0.5.0)

**Theme:** the desktop-app polish phase. This is what makes it feel like a Real App.

### Scope

#### File tree (the centerpiece)
- **`headless-tree`** + **`@tanstack/react-virtual`**
  - React-19-first design, virtualized to 100k+ nodes
  - Replaces stalled `react-arborist`
- **`nucleo = "0.5"`** for in-tree fuzzy filter
  - Helix's matcher; beats fzf algorithm
  - Exposed as Rust → TS via Tauri command (Rust does the matching, TS renders)
- **Tree shows:**
  - File icon (lucide), name, path
  - Per-file: token count, size, redaction badge, compression mode
  - Per-dir: aggregate token count, file count
  - Three-state checkbox: include / exclude / partial-children
- **`globset = "0.4.18"`** for user-typed include/exclude patterns
  - Input field above tree: `src/**/*.rs, !src/test/**`

#### Drag-and-drop
- **Tauri `onDragDropEvent`** for folder drop
  - Returns absolute OS paths (HTML5 `DataTransfer` cannot — this is the killer Tauri-vs-web feature)
  - Drop a folder → auto-set as pack target + populate tree

#### Preview & feedback
- **`shiki = "2.x"`** with JS regex engine for the preview pane
  - TextMate-grade syntax highlighting
  - ~150KB; vs Monaco's 5MB
  - Lazy-load only when preview opens
- **`sonner`** — `toast.promise(packFolder(), { loading, success, error })`
- **`react-resizable-panels`** — split: `tree | preview`

#### Forms & validation
- **`react-hook-form` 7.x** + **`valibot`** + **`@hookform/resolvers`**
  - Uncontrolled forms — no re-render storm
  - Same valibot schema validates UI form *and* preset JSON files (single source of truth)
  - valibot is ~10× smaller than zod
- **Preset files**: JSON-on-disk; load/save through Settings

#### Keyboard & navigation
- **`react-hotkeys-hook` 4.x**
  - `Ctrl+K` — open command palette
  - `Ctrl+S` — save preset
  - `Ctrl+Enter` — start pack
  - `Esc` — cancel
  - Scoped (works inside inputs when scope is whitelisted)
- **`cmdk`** — command palette
  - Jump between recent projects, presets, templates
  - ~5KB; standard desktop polish
- **`@formkit/auto-animate`** — list reorders/additions feel native (2KB)

#### Dev / a11y
- **`@axe-core/react`** (dev-only) — accessibility violations to DevTools
- **`@testing-library/user-event` 14.x** — replaces fireEvent; realistic interactions

### Files affected (Phase 4)

```
frontend/package.json                        # +headless-tree, +@tanstack/react-virtual, +shiki, +react-resizable-panels, +sonner, +react-hook-form, +valibot, +@hookform/resolvers, +react-hotkeys-hook, +cmdk, +@formkit/auto-animate, +@axe-core/react, +@testing-library/user-event
frontend/src/components/tree/                # NEW - virtualized tree + filter input
frontend/src/components/preview/             # NEW - shiki XML preview
frontend/src/components/palette/             # NEW - cmdk command palette
frontend/src/components/dropzone/            # NEW - onDragDropEvent handler
frontend/src/lib/keymap.ts                   # NEW - hotkey definitions
frontend/src/lib/preset-schema.ts            # NEW - valibot schema
crates/core/Cargo.toml                       # +nucleo, +globset
crates/core/src/filter.rs                    # NEW - nucleo matcher exposed
crates/app/src/commands.rs                   # +tree_filter, +load_preset, +save_preset
```

### Acceptance criteria

- [ ] File tree loads & scrolls smoothly with a 50k-file fixture (no jank above 60fps).
- [ ] Dragging a folder into the window populates the target field with the OS-absolute path.
- [ ] `Ctrl+K` opens command palette from any focus state.
- [ ] Saving a preset writes a `.json` file that valibot-validates on next load.
- [ ] XML preview opens within 200ms on a 5MB pack.
- [ ] Pack-in-progress shows a sonner toast that resolves on success/error.
- [ ] axe-core reports zero serious/critical violations on the main route in dev.

### Risks & mitigations

- **`headless-tree` is newer / less battle-tested than react-arborist:** Pin and add a smoke test rendering 50k nodes. Have a fallback plan to a plain virtualized list if blocking.
- **Shiki bundle size on cold start:** Use the `getHighlighter` async path; don't ship all languages — ship only the ones in our tree-sitter set (17).
- **Drag-drop edge cases on Windows:** OneDrive-virtualized folders return synthetic paths. Detect and warn the user before walking.

---

## Phase 5 — Map Mode, Top 4 Languages (v0.6.0)

**Theme:** the differentiator. Big-repo support that doesn't fit even compressed.

### Scope

- **`petgraph = "0.8"`**
- **Symbol graph builder** for Rust, Python, JavaScript, TypeScript only this phase
  - Adapt Aider's `tags.scm` queries per language (Apache 2.0)
  - Per file: extract `(name, kind, line)` tuples for definitions and references
  - Cross-file edges: resolve imports → defs (heuristic, not LSP-grade — module resolution per ecosystem)
- **Personalized PageRank** (~40 LOC over petgraph)
  - Personalization vector seeds: user-selected files (highest weight), open files, recently-edited files
  - Damping 0.85, 50 iterations, convergence at delta <1e-6
  - Output: `Vec<(file_path, score)>` ranked descending
- **Greedy budget filler**
  - Walk ranked list, accumulate tokens until budget exhausted
  - Reserved slots: 10% for stats block, 15% for auto-pinned files, 75% for ranked content
  - Single user-facing knob: `--max-tokens` (UI input)
- **UI: Map Mode toggle**
  - Off → current full-pack behavior
  - On → ranked output with `<repo_map>` section explaining what was included and why
  - Show ranking score in tree column

### Files affected (Phase 5)

```
crates/core/Cargo.toml                       # +petgraph
crates/core/src/map/mod.rs                   # NEW - PageRank + budget filler
crates/core/src/map/extract.rs               # NEW - per-language symbol extraction (4 langs)
crates/core/src/map/resolve.rs               # NEW - cross-file import → def resolver (4 langs)
crates/core/src/grammars/queries/*/tags.scm  # 4 NEW query files (Rust, Py, JS, TS)
crates/core/src/pack/orchestrator.rs         # Map Mode branch
crates/core/src/pack/repo_map.rs             # NEW - <repo_map> emitter
frontend/src/routes/Pack.tsx                 # Map Mode toggle, max-tokens input, ranking display
LICENSE-3RD-PARTY                            # +Aider tags.scm attribution
```

### Acceptance criteria

- [ ] Map Mode on the ProjectPacker repo itself ranks `pack/orchestrator.rs` in the top 5 (it has the most inbound references).
- [ ] Map Mode with a 50k-token budget on a 500k-token repo emits a pack ≤50k tokens.
- [ ] PageRank converges in <1s on a 5000-node graph.
- [ ] Ranked output includes a `<repo_map>` block listing the top 50 nodes with scores.
- [ ] Disabling Map Mode produces byte-identical output to v0.5.0 on a fixture (regression-safe).

### Risks & mitigations

- **Cross-file import resolution is heuristic:** It will be wrong sometimes (especially on dynamic imports, JS aliases). Document this as "best-effort"; PageRank is robust to noisy edges.
- **Symbol extraction queries are per-language brittle:** Insta-snapshot the extracted symbols on a fixture per language; regression-test against query updates.
- **PageRank explodes on circular import graphs:** Damping factor (0.85) handles this naturally; cap iteration count.

---

## Phase 6 — Map Mode All Languages + Plan Envelope + Templates (v0.7.0)

**Theme:** breadth + workflow. Map Mode covers the rest of the language set, the Grok→CC protocol gets the highest-reliability edit format, and prompt UX gets templates.

### Scope

- **Extend symbol graph extraction to remaining 13 languages**
  - Author/adapt `tags.scm` for: Go, Java, C, C++, C#, Ruby, PHP, HTML, CSS, JSON, Markdown, Bash, YAML
  - Languages without meaningful "symbols" (JSON, YAML, MD) fall back to verbatim ranking by reference count
- **SEARCH/REPLACE plan envelope** (Aider format)
  - Highest-reliability edit format on Claude per Aider's polyglot benchmark
  - Trivial Rust parser (~150 LOC): block delimiter detection, file path extraction, before/after text capture
  - No `apply_patch` line-number drift issues
  - Adds to `core/protocol/` alongside existing `validate_plan`
  - UI: pasted plan in Bridge view auto-detects format (existing `grok-to-cc-v1` vs SEARCH/REPLACE)
- **Tera template gallery**
  - `tera = "1"` dep
  - Templates in `crates/core/templates/*.tera`
  - Variables: `{{source_tree}}`, `{{files}}`, `{{git_diff}}`, `{{stats}}`, `{{repo_map}}`
  - Ship 10+ templates:
    - `bug-fix.tera`
    - `refactor.tera`
    - `security-review.tera`
    - `write-tests.tera`
    - `add-feature.tera`
    - `code-review.tera`
    - `migrate-deps.tera`
    - `optimize-perf.tera`
    - `document.tera`
    - `explain.tera`
  - User can add custom templates via Settings → Templates folder

### Files affected (Phase 6)

```
crates/core/Cargo.toml                       # +tera
crates/core/src/grammars/queries/*/tags.scm  # 13 NEW query files
crates/core/src/map/extract.rs               # +13 language extractors
crates/core/src/map/resolve.rs               # +13 language resolvers
crates/core/src/protocol/search_replace.rs   # NEW - parser
crates/core/src/protocol/mod.rs              # router: detect format
crates/core/src/templates/mod.rs             # NEW - Tera engine wrapper
crates/core/templates/*.tera                 # 10+ templates
crates/app/src/commands.rs                   # +list_templates, +render_template
frontend/src/components/templates/           # NEW - template picker
```

### Acceptance criteria

- [ ] All 17 languages produce a Map Mode ranking on a polyglot fixture (mixed-language repo).
- [ ] SEARCH/REPLACE-format plan parses correctly for a 5-block fixture.
- [ ] Plan parser correctly rejects malformed blocks (golden tests for parse errors).
- [ ] Each shipped template renders without Tera errors against a small fixture.
- [ ] Selecting a template in UI substitutes variables and copies to clipboard.

### Risks & mitigations

- **Symbol extraction quality varies wildly per language:** That's fine. PageRank degrades gracefully; languages with no good symbols still rank by file-level reference count.
- **Template variable churn (renaming `{{files}}` etc.) breaks user templates:** Lock variable names at v0.7.0; document in CHANGELOG; never rename without a major-version bump.

---

## Phase 7 — Title-bar + Final Polish + 1.0 Release (v1.0.0)

**Theme:** the polish phase. Everything that says "this is a 1.0 product."

### Scope

#### Custom title-bar (~150-200 LOC realistic)
- Tauri config: `decorations: false`, `transparent: false`, `shadow: true`
- HTML drag region (`data-tauri-drag-region`)
- Three buttons: minimize, maximize/restore, close
  - Lucide icons, hover/active states matching Win11 Mica
  - Right-aligned; macOS uses left-aligned traffic-lights via `tauri::TitleBarStyle::Overlay`
- Window event handler for focused/blurred state
- Double-click drag region → toggle maximize
- Win11 snap layout: `WS_THICKFRAME` retained via Tauri 2's `decorations: false` (verified working)
- Multi-DPI: hit-targets via Tailwind responsive classes
- Accessibility: `role="button"`, `aria-label`s, focus rings

#### Final polish
- App icon set: 256/128/64/48/32/16 PNG + ICO + ICNS
- Branding pass: name, tagline, in-app About dialog
- Full keyboard nav audit: every interactive element is tab-reachable
- Accessibility audit: zero axe-core serious/critical violations
- Empty states: first-run welcome screen, empty-tree state, no-results state
- Error states: friendly messages for permission denied, repo too large, network failure
- Settings → "Clear Cache" button (deferred from Phase 3)
- Settings → "Open Log Folder" button (deferred from Phase 1)
- Settings → "Manage Templates" (deferred from Phase 6)
- Updater (optional): `tauri-plugin-updater` for in-app update checking — only if signing infrastructure is in place

#### Release infrastructure
- Code signing on Windows (cert acquisition + CI integration)
- MSI bundle test on clean Win11 VM
- Portable EXE bundle test
- SHA256SUMS verified by GH Actions release workflow
- `CHANGELOG.md` finalized for 1.0
- README rewritten for a public launch
- Screenshots/GIFs in README

### Files affected (Phase 7)

```
crates/app/src/lib.rs                        # decorations: false, window event handlers
crates/app/src/commands.rs                   # +window_minimize, +window_toggle_maximize, +window_close
frontend/src/components/titlebar/            # NEW - custom title bar component
frontend/src/components/about/               # NEW - About dialog
frontend/src/components/empty-states/        # NEW
frontend/src/styles/                         # final theme tokens
crates/app/icons/                            # full icon set
README.md                                    # public-launch rewrite
CHANGELOG.md                                 # 1.0 finalization
.github/workflows/release.yml                # signed-bundle workflow
```

### Acceptance criteria

- [ ] Custom title-bar: Win11 snap layout shows on hover-maximize.
- [ ] Custom title-bar: double-click drag region toggles maximize.
- [ ] Custom title-bar: focused vs blurred state visually distinct.
- [ ] axe-core: zero serious/critical violations across all routes.
- [ ] Tab order is sensible from window-open to pack-complete.
- [ ] Signed MSI installs cleanly on a fresh Win11 VM, runs without SmartScreen warning post-warmup.
- [ ] Portable EXE runs without admin from a Downloads folder.
- [ ] First-run experience: no presets, no recent projects → welcoming empty state, not a broken-looking app.

### Risks & mitigations

- **Code signing cert acquisition:** EV cert is ~$300-700/year. Plan acquisition 2 weeks before target release. Without signing, SmartScreen requires user to "Run anyway" on first launches.
- **Custom title-bar Win11 edge cases:** Test on at least Win10, Win11 22H2, Win11 23H2. Document known limitations (right-click system menu may not be reachable; live with it or add a hand-rolled fallback menu).
- **Tauri-plugin-updater requires signing infra:** Optional in 1.0; skip if signing isn't ready.

---

## 3. Cross-Cutting Concerns

These don't belong to a single phase but apply throughout.

### Testing strategy

- **Per-phase regression:** every phase adds golden snapshot tests via `insta` for any new emitter or parser.
- **Cross-phase integration:** `pack_integration` test suite grows with each phase; never shrinks.
- **Property tests** via `proptest` for: ignore stacking, secret redactor (no leak across rule boundaries), PageRank convergence on random graphs.
- **Frontend:** Vitest + RTL with `@testing-library/user-event`. Smoke tests per route.
- **Manual checklist** per phase release:
  - Pack the ProjectPacker repo itself
  - Pack a known-large repo (e.g., a Linux kernel snapshot)
  - Pack a polyglot fixture (Rust + Py + TS + Go + Java)
  - Confirm secret redaction on a fake-credential fixture

### Versioning & release cadence

- Each phase = one minor version bump (v0.2.0, v0.3.0, ...).
- Patch releases (v0.2.1, etc.) for hotfixes only.
- v1.0.0 cuts when Phase 7 acceptance criteria met.
- CHANGELOG entries map 1:1 to Phase scope items.
- Tag-triggered release workflow already exists (`.github/workflows/release.yml`) — keep using it.

### Documentation

- `docs/superpowers/specs/` — design docs per phase if scope changes mid-phase.
- `docs/superpowers/plans/` — implementation plans per phase (optional; for complex phases).
- `README.md` — kept lean, points to docs.
- `LICENSE-3RD-PARTY` — vendored attributions (gitleaks, Aider tags.scm, tokenizer JSONs).

### Performance budget

| Operation | Target |
|---|---|
| Cold pack of 10k files | <5s |
| Warm re-pack of 10k files (cache hit) | <500ms |
| File tree render of 50k nodes | 60fps scroll |
| XML preview open on 5MB pack | <200ms |
| Map Mode PageRank on 5k nodes | <1s |
| Secret scan on 100k LOC | <2s |
| Cancellation latency | <200ms |

If a phase's acceptance criteria can't meet the relevant budget, the phase isn't done — fix the perf regression before bumping the version.

### What we're explicitly NOT building

These are documented "no" decisions to prevent scope creep mid-roadmap:

- ❌ MCP server mode (would require headless binary)
- ❌ CLI flags for end users (`projectpacker --map`, etc.)
- ❌ NUL-separated stdin
- ❌ Anthropic Files API integration
- ❌ Any feature requiring an API key
- ❌ Live LLM calls from inside the app
- ❌ Browser/web build (Tauri desktop only)
- ❌ Mac/Linux first-class support before v1.0 (test that they build, but Windows is the launch target)

If any of these come up in scope discussions, reject and link this section.

---

## 4. Dependency Graph Between Phases

```
Phase 1 (foundation)
  ├── Phase 2 (tokens + secrets)         [needs: nothing from 1, parallel possible]
  ├── Phase 3 (compression + cache)      [needs: blake3, ipc::Channel from 1]
  │     └── Phase 5 (Map Mode 4 langs)   [needs: tree-sitter 0.26 + grammars from 3]
  │            └── Phase 6 (Map Mode all + envelope + templates)  [needs: 5]
  │                  └── Phase 7 (1.0)   [needs: everything]
  └── Phase 4 (UX polish)                [needs: shadcn, ipc::Channel from 1]
         └── Phase 7 (1.0)
```

Phases 2 and 4 can run in parallel with later phases if you have parallel work streams. The serial spine is: 1 → 3 → 5 → 6 → 7.

---

## 5. Decision Log

Why these calls were made (so future-me doesn't re-litigate):

| Decision | Rationale |
|---|---|
| GUI-only, no CLI | User constraint. Simplifies build, distribution, support. |
| No API keys | Free-app constraint. Removes secret-management surface, simplifies CI, no rate-limit handling. |
| Vendor 4 tokenizer JSONs (~+25MB) | Free-app + offline-friendly UX. ~$0 hosting cost vs first-run download. Acceptable installer size. |
| Custom title-bar in Phase 7 only | Tar-pit risk; defer until other value shipped. |
| Map Mode split across Phase 5 and 6 | Top 4 languages cover ~80% of repos; ship value before the long tail. |
| Tauri-plugin-log replaces tracing-appender | Strict superset; frontend bridge is a real win. |
| Global cache dir (not per-project) | Cleaner for users; no `.gitignore` churn. Keyed by absolute path so safe. |
| Shippable phases (not layered) | Momentum + dogfooding feedback per release. |
| BLAKE3 over SHA-256 | Content addressing doesn't need crypto guarantees; speed wins. |
| Anthropic cxml schema as default XML format | Documented Anthropic-trained extraction shape; keep legacy schema as setting. |
| `<security_report>` over silent redaction | LLM needs to know "I redacted X here" to reason about gaps. |
| Pin instructional files at top of pack | LLM gets house rules first; biggest UX-per-LOC win in the project. |
| Tail-priority ordering on XML | Matches Anthropic's long-context attention guidance. |

---

## 6. Open Questions (to be resolved before relevant phase)

| Question | Decide before |
|---|---|
| Code-signing cert source (EV vs OV vs none for v1.0)? | Phase 7 |
| LRU cache eviction policy: 2GB, 30 days, or both? | Phase 3 |
| Which 10+ Tera templates ship by default — final list? | Phase 6 |
| Do we support Mac/Linux as "best-effort builds" at v1.0, or label as Windows-only? | Phase 7 |
| Should there be an in-app log viewer (using tauri-plugin-log frontend bridge)? | Phase 4 or skip |

---

**End of plan.**
