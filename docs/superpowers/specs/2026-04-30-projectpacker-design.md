# ProjectPacker — Design Document

**Status:** Draft, awaiting user approval
**Date:** 2026-04-30
**Author:** Brainstorming session (Claude + Carter)
**Successor to:** CodeParser (V1) — `e:\Tools\Parsers\CodeParser`
**Target repo:** `github.com/CBaileyDev/ProjectPacker`

---

## 1. Summary

ProjectPacker is a Windows desktop app that turns a local folder or GitHub repository into a single self-describing XML "pack" file optimized for a two-AI workflow: a **planner** (Grok, web-based, multi-agent) reads the pack and produces a strict-format change plan with explicit per-step rationale; an **executor** (Claude Code, running directly in the target repo) reviews the plan, may challenge any step, and then carries it out. ProjectPacker itself never edits files — it is a packer, a protocol layer, and a validator that bridges the two AIs.

ProjectPacker is the successor to V1 (CodeParser), reimplemented as a Tauri desktop app with a Rust core and a React + TypeScript front end. The rewrite delivers (a) a much higher visual ceiling for the UI, (b) a clean separation of the parsing core from the shell so it can be reused later, and (c) the new structured Grok ↔ Claude Code handoff that V1 cannot do.

## 2. Goals

- **Pack any local folder or GitHub URL** into a Repomix-style XML file, fully respecting `.gitignore`, `.codeparserignore`, and built-in defaults.
- **Embed a strict protocol** in the pack telling the planner AI exactly how to structure its response, including a mandatory `Rationale` per step.
- **Generate a Claude Code prompt** that instructs the executor AI to read the full plan, challenge weak rationales before executing, and verify after each step.
- **Validate planner output** before it reaches the executor — catch malformed plans early and regenerate them.
- **Provide a visually polished, animation-rich UI** with room for design treatments well beyond V1.
- **Match V1's feature set:** ignore handling, token counting, secret scanning, optional Tree-sitter compression, optional comment removal, optional git history, drag-and-drop, GitHub URL clones.
- **Ship as a single Windows installer + portable .exe** with no runtime dependencies.

## 3. Non-goals (v1.0)

- ProjectPacker does **not** apply changes to files. Claude Code does that.
- No built-in chat panel — users bring their own AI (Grok web, Claude.ai, etc.) via copy-paste.
- No CLI binary in v1.
- No macOS / Linux builds in v1.
- No code signing in v1 (Windows SmartScreen warning is documented).
- No auto-update in v1 (manual download from GitHub Releases).
- No telemetry or network calls except optional GitHub clones.
- No multiple concurrent pack jobs.
- No plugin system for handoff protocols (single `grok-to-cc-v1` shipped).

## 4. Key design decisions

| # | Question | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | Form factor | Tauri desktop app | Maximum UI ceiling, small binary, native feel, cross-platform-ready |
| 2 | Parsing core | Full Rust rewrite (no Python sidecar) | One self-contained binary; no Python runtime; long-term clean foundation |
| 3 | Workflow shape | Pack → Grok plans → Claude Code executes | Plays to each AI's strengths: Grok's multi-agent web reasoning + Claude Code's local file access and execution |
| 4 | Plan format | Strict markdown schema with mandatory `Rationale` per step | Catches Grok errors early; lets Claude Code make informed second-opinion decisions |
| 5 | AI integration | Clipboard / file only ("BYO AI") | Zero auth, zero billing, works with any AI the user already pays for |
| 6 | Platform | Windows only for v1 | Mirrors V1's audience; cross-platform deferred |
| 7 | CLI | None in v1 | GUI-only keeps scope tight |
| 8 | Frontend stack | React 19 + TypeScript + Vite + Tailwind v4 | Largest motion/animation/component ecosystem (Framer Motion, R3F, shadcn, Aceternity, etc.); Tauri removes bundle-size penalty |
| 9 | Architecture | Layered: `crates/core` (lib) + `crates/app` (Tauri shell) + streaming events + specta-typed bridge | Reusable core, real-time UI progress, type-safe Rust↔TS contract |
| 10 | UI design source | Hand off to Claude Design (claude.ai/design) for components | Plays to Claude Code's strengths (Rust core, plumbing) and Claude Design's strengths (polished React surface) |

## 5. Architecture

### 5.1 High-level

```
┌─────────────────────────────────────────────────────────────┐
│                    ProjectPacker (Tauri app)                │
│                                                             │
│  ┌──────────────────────┐      ┌──────────────────────┐     │
│  │  React 19 + TS UI    │      │   Rust workspace     │     │
│  │  (Vite, Tailwind v4) │◄────►│                      │     │
│  │                      │ Tauri│  ┌────────────────┐  │     │
│  │  - Pack screen       │ event│  │ crate: app     │  │     │
│  │  - Goal input        │ bus  │  │ (Tauri shell,  │  │     │
│  │  - Progress / anim   │ +    │  │  commands,     │  │     │
│  │  - Result + copy     │ typed│  │  event emit)   │  │     │
│  │  - Bridge tab        │ cmds │  └────────┬───────┘  │     │
│  │  - Settings          │      │           │ depends  │     │
│  │                      │      │  ┌────────▼───────┐  │     │
│  │  Auto-gen TS types   │      │  │ crate: core    │  │     │
│  │  via specta          │      │  │ (pure Rust)    │  │     │
│  │                      │      │  │                │  │     │
│  │                      │      │  │  walker        │  │     │
│  │                      │      │  │  ignore        │  │     │
│  │                      │      │  │  pack          │  │     │
│  │                      │      │  │  protocol      │  │     │
│  │                      │      │  │  secrets       │  │     │
│  │                      │      │  │  tokens        │  │     │
│  │                      │      │  │  tree_sitter   │  │     │
│  │                      │      │  │  github (gix)  │  │     │
│  │                      │      │  └────────────────┘  │     │
│  └──────────────────────┘      └──────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 Process model

Single Tauri process. Rust core runs in-process on a Tokio runtime; heavy work (file walking, hashing, secret scanning, tokenizing) runs on `tokio::task::spawn_blocking` or `rayon::par_iter` to keep the UI responsive. Long-running pack operations emit progress events on the Tauri event bus (`pack:{job_id}:progress`) which React subscribes to. Each pack has a unique `JobId`; only one pack job runs at a time in v1.

### 5.3 Source layout

```
ProjectPacker/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── core/                     # pure-Rust library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── walker.rs
│   │       ├── ignore.rs
│   │       ├── pack/
│   │       │   ├── mod.rs
│   │       │   └── xml.rs
│   │       ├── protocol.rs       # used by both pack and Bridge tab
│   │       ├── secrets.rs
│   │       ├── tokens.rs
│   │       ├── tree_sitter.rs
│   │       ├── github.rs
│   │       ├── types.rs
│   │       └── error.rs
│   └── app/                      # Tauri shell
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── build.rs              # specta type emission
│       ├── icons/
│       └── src/
│           ├── main.rs
│           ├── commands.rs
│           ├── events.rs
│           └── settings.rs
├── frontend/
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── tailwind.config.ts
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── bindings/             # auto-generated, gitignored
│       ├── lib/
│       │   ├── api.ts
│       │   ├── events.ts
│       │   └── store.ts
│       ├── components/
│       ├── routes/
│       │   ├── Home.tsx
│       │   ├── Pack.tsx
│       │   ├── Result.tsx
│       │   ├── Bridge.tsx
│       │   └── Settings.tsx
│       └── styles/globals.css
├── docs/
│   ├── superpowers/specs/
│   │   └── 2026-04-30-projectpacker-design.md
│   ├── superpowers/plans/
│   └── protocol/
│       └── grok-to-cc-v1.md
├── tests/
│   ├── fixtures/
│   │   ├── tiny/
│   │   ├── medium/
│   │   └── binary/
│   └── e2e/
├── scripts/
│   └── build-release.ps1
├── .github/workflows/
│   ├── ci.yml
│   └── release.yml
├── README.md
├── CHANGELOG.md
└── LICENSE
```

### 5.4 Key technology choices

| Concern | Crate | Why |
|---|---|---|
| Async runtime | `tokio` | Standard, integrates with Tauri |
| File walking | `ignore` (BurntSushi) | Best-in-class `.gitignore` semantics |
| Git / GitHub clone | `gix` (gitoxide) | Pure Rust, no system git, supports shallow clone |
| Tokenizing | `tiktoken-rs` | Matches V1 |
| Secret scanning | Pure-Rust regex with gitleaks rule patterns ported | Replaces V1's `detect-secrets` |
| Tree-sitter | `tree-sitter` + per-language grammars | Matches V1's compression |
| XML emission | `quick-xml` | Streaming, Repomix-compatible |
| TS bindings | `specta` + `tauri-specta` | Auto-generates `bindings/*.ts` from Rust |
| Persistence | `serde_json` to AppData | Settings/recents are tiny |

## 6. Components

### 6.1 Rust core (`crates/core`)

#### 6.1.1 `types.rs` — shared data shapes

The interface contract for the entire app. Every type crossing the Rust↔TS boundary lives here so `specta` can derive both Rust serde and TypeScript definitions.

```rust
pub struct PackOptions {
    pub target: PackTarget,            // Folder(PathBuf) | GitHub(String)
    pub goal: String,
    pub include_git_history: bool,
    pub count_tokens: bool,
    pub tokenizer_model: String,       // default: "gpt-4o-mini"
    pub secret_scan: bool,
    pub compress: bool,
    pub remove_comments: bool,
    pub max_file_size_kb: u32,         // default: 1024
    pub respect_gitignore: bool,
    pub custom_ignore_patterns: Vec<String>,
    pub protocol_version: String,      // default: "grok-to-cc-v1"
}

pub struct PackResult {
    pub xml: String,
    pub claude_code_prompt: String,
    pub stats: PackStats,
    pub warnings: Vec<PackWarning>,
}

pub struct PackStats {
    pub files_total: u32,
    pub files_included: u32,
    pub files_skipped: u32,
    pub bytes_total: u64,
    pub tokens_total: Option<u32>,
    pub secrets_found: u32,
    pub duration_ms: u32,
}

pub enum ProgressEvent {
    Started { job_id: String, target_label: String },
    Cloning { progress_pct: u8 },
    Walking { files_scanned: u32 },
    FileFoundBatch { paths: Vec<FileFound> },
    FileSkipped { path: String, reason: SkipReason },
    Tokenizing { progress_pct: u8 },
    SecretScanning { progress_pct: u8 },
    SecretHit { path: String, kind: String, line: u32 },
    Compressing { progress_pct: u8 },
    BuildingXml,
    Done(PackStats),
    Error { message: String, fatal: bool },
}
```

#### 6.1.2 `walker.rs` — async file walker

Wraps the `ignore` crate behind an async stream. Emits `FileFoundBatch` events (throttled to 50ms or 100 files). Handles symlinks (skip by default), max-depth, and respects `max_file_size_kb`.

#### 6.1.3 `ignore.rs` — ignore rules

Builds the matcher: `.gitignore` (recursive) + `.codeparserignore` (project root) + built-in defaults (`node_modules/`, `.git/`, `target/`, `dist/`, lockfiles, common image/video extensions) + user's custom patterns. Single source of truth — every component asks `ignore.is_ignored(path)`.

#### 6.1.4 `pack/xml.rs` — XML builder

Streaming `quick-xml` writer producing a Repomix-compatible structure:

```xml
<repository>
  <protocol version="grok-to-cc-v1">…</protocol>     <!-- new in V2 -->
  <user_task>…</user_task>                            <!-- new in V2 -->
  <file_summary>…</file_summary>
  <directory_structure>…</directory_structure>
  <files>
    <file path="…" tokens="…" hash="…">…</file>
  </files>
  <git_logs>…</git_logs>
</repository>
```

Each file's `hash` is SHA-256 of its content (enables future drift detection). Each file's `tokens` count is included when `count_tokens=true`.

#### 6.1.5 `protocol.rs` — the protocol module

The heart of V2. Top-level module (not under `pack/`) because it's used both during packing and by the Bridge tab. Exposes:

- `block_for_pack(goal: &str, version: &str) -> String` — builds the `<protocol>…</protocol>` block embedded in the pack XML.
- `claude_code_prompt(version: &str) -> String` — returns the CC prompt template for that version.
- `validate_plan(md: &str, version: &str) -> Result<(), Vec<PlanError>>` — checks a Grok response against the protocol grammar.
- `build_combined_prompt(plan_md: &str, version: &str) -> String` — wraps a validated plan with the CC prompt template.

Templates are loaded via `include_str!` from `docs/protocol/grok-to-cc-v{N}.md` so they're versioned, reviewable, and frozen on release. Full spec in §8.

#### 6.1.6 `secrets.rs` — secret scanner

Pure-Rust regex pipeline using gitleaks rule patterns (AWS keys, GitHub tokens, generic API keys, private keys). Runs on file content before XML emission. Default: warn but do not redact (redacting code can break LLM understanding); option exists to redact if user prefers.

#### 6.1.7 `tokens.rs` — token counter

`tiktoken-rs` wrapper. Encoder initialized once in a `OnceLock` and reused. Counts per-file (cached) and total. Parallelized across files with `rayon`.

#### 6.1.8 `tree_sitter.rs` — compression

Optional. For supported languages (Rust, Python, JS/TS, Go, Java, C, C++), parses with tree-sitter and emits a "skeleton": signatures, types, top-level structure, with bodies elided. Falls back to raw content for unsupported languages or parse failures.

#### 6.1.9 `github.rs` — GitHub URL handling

Uses `gix` to perform shallow clone (`--depth 1`) into a temp directory. Emits `Cloning` events. Returns the temp path; an RAII guard cleans up the temp dir when the `PackJob` is dropped.

### 6.2 Tauri app (`crates/app`)

#### 6.2.1 `commands.rs` — exposed Tauri commands

| Command | Args | Returns |
|---|---|---|
| `pack_start` | `PackOptions` | `JobId` |
| `pack_cancel` | `JobId` | `()` |
| `pack_get_result` | `JobId` | `PackResult` |
| `validate_plan` | `plan_md: String, protocol_version: String` | `PlanValidation` |
| `build_combined_prompt` | `plan_md: String, protocol_version: String` | `String` |
| `select_folder` | — | `Option<PathBuf>` |
| `save_to_file` | `path: PathBuf, contents: String` | `()` |
| `copy_to_clipboard` | `text: String` | `()` |
| `get_settings` / `save_settings` | `Settings` | `Settings` |

`pack_start` spawns a Tokio task and returns immediately; the task emits progress events keyed to `JobId`.

#### 6.2.2 `events.rs` — typed event helpers

Thin emitter wrappers ensuring every event is properly serialized and namespaced.

#### 6.2.3 `settings.rs` — persistence

Reads/writes `%APPDATA%/ProjectPacker/settings.json`:
- Recents (last 10 targets)
- Saved presets (named option sets)
- UI prefs (theme, default protocol version, default tokenizer model)
- Default goal templates

JSON file. No DB. ~5 KB max.

### 6.3 React UI (`frontend/`)

Five routes:

- **`Home`** — landing screen with "New Pack" CTA and recents list.
- **`Pack`** — target picker (folder drop or GitHub URL), goal editor, options panel (toggles + advanced disclosure + presets), pack button, live progress panel during pack.
- **`Result`** — shown after `Done`. Pack stats header, "Copy Pack XML" + "Copy Claude Code Prompt" + "Save as…" buttons, tabbed views for `Pack XML` / `CC Prompt` / `Warnings` / `Skipped Files`.
- **`Bridge`** — paste Grok's plan; ProjectPacker validates against the protocol grammar, shows specific errors if malformed, generates a re-prompt for Grok to fix it; on success, wraps the plan with the Claude Code prompt template and exposes a single "Copy combined prompt" button.
- **`Settings`** — manage presets, edit ignore defaults, change theme, set default tokenizer model, edit goal templates.

State is held in a Zustand store (`lib/store.ts`). Typed API wrappers (`lib/api.ts`) call `invoke()` using auto-generated `bindings/` types. Event subscription helpers (`lib/events.ts`) wrap Tauri's event bus.

## 7. Data flow

### 7.1 Happy-path sequence

```
┌────────────┐                ┌──────────────┐                ┌──────────────┐
│   React    │                │  Tauri app   │                │  core crate  │
│    UI      │                │   crate      │                │              │
└─────┬──────┘                └──────┬───────┘                └──────┬───────┘
      │                              │                               │
      │  invoke("pack_start", opts)  │                               │
      ├─────────────────────────────►│                               │
      │                              │  spawn Tokio task             │
      │ ◄────────── JobId ───────────┤                               │
      │                              │                               │
      │  listen("pack:{id}:progress")│                               │
      ├─────────────────────────────►│                               │
      │                              │  build IgnoreMatcher          │
      │                              ├──────────────────────────────►│
      │ ◄──── Started event ─────────┤                               │
      │  animate "starting…"         │  walker.stream()              │
      │                              ├──────────────────────────────►│
      │ ◄── FileFoundBatch (50ms) ───┤  ◄── stream of files          │
      │  ticker scrolls              │                               │
      │                              │  rayon: tokenize files        │
      │ ◄──── Tokenizing % ──────────┤                               │
      │                              │  rayon: scan secrets          │
      │ ◄──── SecretHit ─────────────┤                               │
      │  warning banner appears      │  build XML + protocol         │
      │                              ├──────────────────────────────►│
      │ ◄──── BuildingXml ───────────┤                               │
      │                              │  store PackResult by JobId    │
      │ ◄──── Done(stats) ───────────┤                               │
      │  navigate to /result         │                               │
      │  invoke("pack_get_result")   │                               │
      ├─────────────────────────────►│                               │
      │ ◄── PackResult ──────────────┤                               │
      │                              │                               │
      │  user clicks "Copy Pack XML" │                               │
      │  invoke("copy_to_clipboard") │                               │
      ├─────────────────────────────►│                               │
```

### 7.2 Phase-by-phase

1. **Setup.** UI calls `pack_start(opts)`. App validates `opts`, generates UUID v7 `JobId`, spawns Tokio task, returns `JobId` immediately. UI navigates to progress view and subscribes to events.
2. **Source acquisition.** If `target = GitHub`, `gix` shallow-clones into `%TEMP%/projectpacker/{job_id}/` with progress callbacks. RAII guard cleans up on drop. If `target = Folder`, skip.
3. **File discovery.** Build `IgnoreMatcher`. Walk with `ignore::WalkBuilder`. Per file: emit `FileSkipped` (ignored / too-large / binary heuristic via NUL byte in first 8KB) or push to `Vec<FileEntry>`. Throttle `FileFound` into 50ms / 100-file batches.
4. **Per-file processing.** `rayon::par_iter`: tokenize, secret-scan, tree-sitter compress, comment-strip. Stage progress reflects slowest concurrent stage.
5. **Pack assembly.** Build `<protocol>` block from template + goal. Stream XML: protocol → user_task → file_summary → directory_structure → files (each with hash + tokens) → optional git_logs. Build `claude_code_prompt` separately.
6. **Result retrieval.** Pack stored in in-memory map keyed by `JobId`. UI calls `pack_get_result`. Result is **not** persisted; closing app discards it. User must explicitly copy or save.
7. **User actions.** Copy XML → clipboard. Copy CC prompt → clipboard. Save → native dialog. New Pack → return to /pack with options preserved.

### 7.3 Bridge flow

Separate from pack flow:

```
User runs Grok in browser → copies Grok's plan markdown
   │
   ▼
Bridge tab: paste plan into textarea
   │
   ▼
Frontend calls invoke("validate_plan", { plan_md, protocol_version })
   │
   ├─ Invalid → render error list + "Copy re-prompt for Grok" button
   │
   └─ Valid → invoke("build_combined_prompt", …) returns wrapped prompt
                │
                ▼
       Render "Copy combined prompt" button
                │
                ▼
       User pastes into Claude Code session in target repo
```

### 7.4 Cancellation

`pack_cancel(jobId)` looks up the `JoinHandle` and `.abort()`s it. Tokio task interrupts at next `.await`; RAII drops clean up temp dirs. App emits `Error { message: "Cancelled by user", fatal: true }`. UI returns to /pack.

### 7.5 Pack job state machine

```
        ┌──► Cloning ──┐
        │              ▼
  Pending ─────────► Walking ──► Processing ──► Building ──► Done
        │              │              │              │
        └──────────────┴──────────────┴──────────────┴────► Cancelled
                                                            Errored
```

A job is in exactly one state at a time; UI animates against transitions.

## 8. Protocol specification

The protocol is the heart of V2. Three artifacts live in `docs/protocol/grok-to-cc-v1.md` and are baked into the binary at compile time.

### 8.1 Versioning

- Versioned strings: `grok-to-cc-v1`, future `grok-to-cc-v2`, etc.
- Each version is a frozen markdown file. Once shipped, never edited — only superseded.
- Pack records its version in `<protocol version="…">` and in `claude_code_prompt`. Mismatched versions warn loudly.
- Default = latest bundled. Settings allows pinning an older version per project.

### 8.2 Three artifacts

| Artifact | Audience | Where it lives | Purpose |
|---|---|---|---|
| Pack protocol block | Grok (planner) | Top of pack XML | Tells Grok: don't write code, produce strict-format plan with rationales |
| Plan format spec | Grok (planner) | Inside the protocol block | Defines exact markdown structure Grok must emit |
| Claude Code prompt | Claude Code (executor) | Returned alongside pack | Tells CC: read fully, challenge rationales before executing, verify after each step |

### 8.3 Pack protocol block (verbatim text)

```
<protocol version="grok-to-cc-v1">
You are reading a snapshot of a software project. Your role in this
workflow is PLANNER. You will NOT write the code yourself. Another AI
agent (Claude Code) is operating directly inside this repository and will
execute your plan, with the right to challenge any step.

Your output must follow the PLAN FORMAT below exactly. Plans that deviate
will be rejected by the validator and the user will paste them back to
you for correction.

## Workflow context

1. The user has a goal, stated in the <user_task> block below.
2. You read the codebase and the goal.
3. You produce a plan: a sequence of concrete steps that, taken together,
   accomplish the goal.
4. For every step, you must include a `Rationale` explaining WHY that
   step is needed. Claude Code will read your rationale and may challenge
   any step it disagrees with before executing — provide enough reasoning
   for an informed second opinion.
5. The user pastes your plan into Claude Code. Claude Code reviews the
   full plan, challenges any weak rationale, and executes the rest.

## What you can ask Claude Code to do

- Edit a specific file (provide enough context that the edit is unambiguous)
- Create a new file (provide its full intended contents or a clear specification)
- Delete a file
- Rename or move a file
- Run a command (tests, linters, build, migrations)

## Plan format (STRICT)

Your response must be a single Markdown document with these sections in
this order:

### Summary
One short paragraph (≤4 sentences) describing the overall approach.

### Risks
A bulleted list of risks or open questions Claude Code should be aware
of before executing. May be empty (`- None.`).

### Steps
A numbered list. Every step is an H4 (`#### Step N: …`) and includes
EXACTLY these fields, in this order, each on its own line:

  **Action:** edit | create | delete | rename | run
  **Target:** <file path relative to repo root, OR shell command if
              Action is `run`>
  **Rationale:** <one or two sentences. WHY this step is needed.
                  Claude Code uses this to decide whether to challenge.>
  **Details:**
  <freeform body — code blocks, diffs, full file contents, or prose
   describing the change. Use ```lang fenced blocks for code.>

### Verification
A bulleted list of how Claude Code should verify the plan succeeded
(commands to run, things to check). At least one item.

### Rollback
A bulleted list of how to undo the change if needed. May be `- Use git
to revert.` if no special steps.

## Hard rules

- Do NOT include any prose outside the sections above.
- Do NOT propose changes to files not present in this pack.
- Do NOT use the words "you should" or "consider" in Rationale —
  state the reason as a fact.
- Every Step MUST have a non-empty Rationale.
- If you are unsure about something, put it in Risks instead of guessing.
</protocol>
```

### 8.4 Plan format example (for reference)

````markdown
### Summary
Add MFA support to the login flow. Introduce a new `mfa` module, wire it
into `login()`, and update the user model to track MFA enrollment state.

### Risks
- The session token format changes; existing sessions will be invalidated.
- The `mfa.py` module assumes TOTP — SMS fallback is out of scope.

### Steps

#### Step 1: Add MFA enrollment field to User model
**Action:** edit
**Target:** src/models/user.py
**Rationale:** The user model is the single source of truth for auth state; without an enrollment field, login() cannot branch on whether MFA is required.
**Details:**
Add a new column:
```python
mfa_enrolled: bool = Field(default=False)
mfa_secret: Optional[str] = Field(default=None)
```

#### Step 2: Create the mfa module
**Action:** create
**Target:** src/auth/mfa.py
**Rationale:** Isolating MFA logic in its own module keeps login() readable and lets us swap TOTP for WebAuthn later without touching the login path.
**Details:**
```python
import pyotp

def verify_totp(secret: str, code: str) -> bool:
    return pyotp.TOTP(secret).verify(code, valid_window=1)
```

#### Step 3: Wire MFA into login
**Action:** edit
**Target:** src/auth/login.py
**Rationale:** This is the actual integration point. Branching here ensures MFA is enforced before a session is issued, not after.
**Details:**
Modify `login()` so that if `user.mfa_enrolled` is true, a `mfa_code`
parameter is required and verified before the session is issued.

#### Step 4: Run the test suite
**Action:** run
**Target:** pytest tests/auth/
**Rationale:** Auth changes have a high blast radius; running the existing auth tests before declaring done catches regressions early.
**Details:**
Expect all existing tests to pass. New MFA-specific tests come in a follow-up plan (out of scope here).

### Verification
- `pytest tests/auth/` passes.
- A user with `mfa_enrolled=False` can still log in normally.
- A user with `mfa_enrolled=True` is rejected without `mfa_code`.

### Rollback
- `git revert` the resulting commits — no DB migration in this plan.
````

### 8.5 Claude Code prompt template (verbatim text)

```
You are operating directly inside the repository this plan refers to.
You have full file access — use it.

Below is a plan produced by a planner AI (Grok) using protocol version
grok-to-cc-v1. Your role in this workflow is EXECUTOR with veto power.

## How to handle this plan

1. **Read the entire plan first.** Don't start executing step 1 until
   you've read every step, the Risks section, and the Verification
   section.

2. **Evaluate every Rationale.** For each step, decide whether the
   rationale holds given what you can see in the actual repo. You have
   context the planner did not — files may have changed, the planner
   may have misread the codebase, or there may be a simpler approach.

3. **Challenge before executing.** If you disagree with a step, STOP
   and tell the user:
   - Which step you disagree with.
   - What the planner's rationale was.
   - Why you think it is wrong or suboptimal.
   - What you propose instead.
   Wait for the user's decision before proceeding.

4. **Execute step-by-step, not all at once.** After each step:
   - Run any obvious verification (the file compiles, imports resolve,
     a quick targeted test passes).
   - If something fails or looks wrong, stop and report. Do not paper
     over a failing step to keep the plan moving.

5. **Run the Verification section at the end.** Report the result of
   each item.

6. **Stay within scope.** Do not refactor adjacent code, fix unrelated
   bugs, or add features beyond what the plan specifies — even if you
   notice issues. Mention them in your final summary instead.

## Plan follows

---

[The plan from Grok will be inserted here by the Bridge step.]
```

### 8.6 Validator rules

`core::protocol::validate_plan(md, version)` checks:

| Rule | Failure message |
|---|---|
| Has `### Summary`, `### Risks`, `### Steps`, `### Verification`, `### Rollback` headings, in order | "Missing or out-of-order section: ___" |
| Every `#### Step N:` has Action, Target, Rationale, Details fields | "Step N is missing field: ___" |
| Action is one of `edit \| create \| delete \| rename \| run` | "Step N has invalid Action: ___" |
| Rationale is non-empty (≥10 chars) | "Step N has empty Rationale" |
| Verification has ≥1 item | "Verification section is empty" |
| No prose outside the five top-level sections | "Unexpected text before/between sections" |

On failure, the Bridge tab renders the error list and offers a copy button for a re-prompt: "Your previous response failed validation. Errors: ___. Please re-emit following protocol grok-to-cc-v1 exactly."

## 9. Error handling

### 9.1 Two-axis classification

|  | **Recoverable** (pack continues, file/step skipped) | **Fatal** (pack aborts) |
|---|---|---|
| **Per-file** | bad encoding, tree-sitter parse fail, file vanished mid-walk, regex timeout | — |
| **Per-pack** | git log failed → omit `<git_logs>` | invalid target, clone failed, OOM during XML build |
| **Per-command** | clipboard write failed | settings file corrupt, can't reach AppData |

### 9.2 Error types

`thiserror`-derived `CoreError` per crate; `anyhow` only at command boundaries. At the Tauri command boundary, errors convert to a serializable `AppError { code, message, details }` for React.

### 9.3 Pipeline failures (highlights)

- **Target validation:** fatal, rejected synchronously in `pack_start`.
- **GitHub clone:** fatal, with distinct error codes (`CloneNetwork`, `CloneNotFound`, `CloneAuth`, `CloneOther`).
- **Walker per-file errors:** recoverable, emit `PackWarning::Inaccessible`, skip and continue.
- **Encoding:** try `encoding_rs` fallbacks (UTF-16, Windows-1252); skip with warning if still fails.
- **Tree-sitter parse:** recoverable, fall back to raw content. Warning only if `compress=true`.
- **Tokenizer unavailable:** fatal at first file, clear message.
- **Git log:** recoverable, omit section, warn.
- **Cancellation:** fatal, RAII cleans up.
- **Settings corrupt:** recoverable, back up bad file, start fresh, toast.
- **Clipboard:** recoverable, toast.

### 9.4 UI surfacing

- **Inline** (form validation): bad URL, missing path, invalid regex pattern.
- **Toast** (transient): copy success/failure, settings recovered, single-at-a-time queue.
- **Modal/banner** (job-level): "Pack failed" panel with error code + human message + "Show details" disclosure + "Try Again" + "Copy Error Report" buttons; warnings card on result screen if pack succeeded with warnings.

### 9.5 Logging

JSON-line logs at `%APPDATA%/ProjectPacker/logs/projectpacker.log`. Rolling, 10 MB × 3 files. Settings has "Open Log Folder" and "Copy Last Error" buttons.

### 9.6 Panic policy

Rust core is panic-free by policy: no `unwrap`/`expect` outside tests, no array indexing without bounds check, regex compilation in `OnceLock`. App crate sets `std::panic::set_hook` that catches any panic, logs the backtrace, emits a fatal `AppError { code: InternalPanic, … }`, and continues running. The app does not crash.

### 9.7 No-go list

- No telemetry / error reporting to a server.
- No automatic retries (user decides).
- No silent fallbacks for fatal-class errors (e.g., we don't quietly disable token counting).

## 10. Testing

### 10.1 Pyramid

```
              ┌──────────────┐
              │   E2E (1-2)  │   ← Playwright drives the Tauri app
              ├──────────────┤
              │ Integration  │   ← core crate against fixture repos
              │   (~30)      │
              ├──────────────┤
              │   Unit       │   ← per-module Rust tests, plus
              │  (~150-200)  │     vitest for UI lib code
              └──────────────┘
```

Heavy on unit + integration in Rust, light on UI tests.

### 10.2 Rust unit tests

Standard `#[cfg(test)]` modules per file. Coverage targets: walker (depth, symlinks, max-size), ignore (gitignore precedence, custom patterns), pack/xml (ordering, escaping, attributes), protocol (`block_for_pack` template substitution + version pinning, `claude_code_prompt` returns frozen text, `validate_plan` every rule pass + fail, `build_combined_prompt` round-trip), secrets (each rule pattern, positive + negative), tokens, tree_sitter (per language: signatures preserved, bodies elided), github (URL parsing variants).

### 10.3 Integration tests (`crates/core/tests/`)

End-to-end pack against fixture repos:

| Fixture | Files | Purpose |
|---|---|---|
| `tiny/` | ~10, 3 dirs, 1 nested .gitignore, 1 secret-looking string | every stage exercised quickly |
| `medium/` | ~150, multi-language, realistic ignore tree, fake `node_modules/` | regression coverage |
| `binary/` | binary + non-UTF-8 + oversize | binary detection, encoding fallback |

Each test snapshots `PackStats` and the resulting XML. Snapshot review via `cargo insta`. Snapshots are committed; CI fails on drift.

### 10.4 Golden file tests for the protocol

`crates/core/tests/protocol_golden.rs` snapshots:
- Full `<protocol>` block for each version.
- Claude Code prompt template.
- Fully-built pack XML for `tiny/` with goal "Add a hello endpoint."
- A known-good plan parsing through `validate_plan`.
- Ten known-bad plans (one per validator rule) with their exact error messages.

**Once a protocol version is released, its snapshots are frozen forever.** Any change to a released version's output fails CI hard. New behavior goes in a new version.

### 10.5 Property tests

Using `proptest`: ignore matcher invariants; walker enumeration determinism; XML escape round-trip. 256 cases each; not more.

### 10.6 Tauri command tests

`crates/app/tests/commands.rs` using `tauri::test::mock_builder`. Round-trip every command without launching the WebView.

### 10.7 Frontend tests

Deliberately minimal: vitest for `lib/` (api wrappers, event sub/unsub, store reducers); one smoke test that each route renders without errors. **No component snapshot tests** — visual treatment will change continuously.

### 10.8 E2E tests

One Playwright test on the happy path: launch app → pick fixture → goal → pack → copy buttons → bridge tab valid plan → bridge tab invalid plan errors render. Catches integration breakage, not component coverage.

### 10.9 CI

GitHub Actions Windows-only (`.github/workflows/ci.yml`):

```
on: [push, pull_request]
jobs:
  test:
    runs-on: windows-latest
    steps:
      - checkout
      - install rust (stable, rustfmt, clippy)
      - install node 20 + pnpm
      - cargo fmt --check
      - cargo clippy --workspace --all-targets -- -D warnings
      - cargo test --workspace
      - cd frontend && pnpm install --frozen-lockfile
      - pnpm typecheck && pnpm test
      - pnpm e2e   # gated to release branches
```

### 10.10 Test data hygiene

Fixtures use synthetic content. Secret-detection fixtures use deliberately-invalid-but-shaped keys (`AKIA0000000000000000`). `.gitleaks.toml` allowlists fixture paths.

### 10.11 Explicitly out of scope

No screenshot tests. No Storybook. No Chromatic. No network-dependent GitHub clone tests (use local file:// fake remotes via gix). No tokenizer-accuracy tests beyond "returns a number."

## 11. Packaging & distribution

### 11.1 Build outputs

| Artifact | Format | Size estimate |
|---|---|---|
| `ProjectPacker_x.y.z_x64-setup.msi` | MSI installer | ~15-25 MB |
| `ProjectPacker_x.y.z_x64-portable.exe` | Self-contained .exe | ~12-20 MB |

### 11.2 Build pipeline

Local dev: `pnpm tauri dev` (hot reload).
Local release: `.\scripts\build-release.ps1 0.1.0` runs tests + bundles MSI + portable.
CI release (`.github/workflows/release.yml`): tag-triggered on `v*.*.*`, runs all tests, builds bundles, creates GitHub Release with notes from `CHANGELOG.md`, uploads `.msi`, `.exe`, and `.sha256` files.

### 11.3 Versioning

SemVer. `0.1.0` first preview → `0.x` until protocol-v1 has been used in real workflows for a few weeks → `1.0.0`. Version set in `crates/app/Cargo.toml`; pre-build script propagates to `frontend/package.json`, `tauri.conf.json`, workspace.

### 11.4 Auto-update

**Not in v1.** About screen has "Check for updates" button hitting `api.github.com/repos/CBaileyDev/ProjectPacker/releases/latest`. No background polling. Tauri updater plugin added in v0.5+ with Ed25519-signed manifests.

### 11.5 Code signing

**Not in v1.** Documented SmartScreen warning + published SHA256 hashes. Sign once user count justifies the $200-500/year cert.

### 11.6 Bundled resources (compile-time embedded)

Protocol templates, ignore defaults, tree-sitter grammars, tiktoken encodings, icons, secret-scanner rules. Nothing pulled at runtime.

### 11.7 Install footprint

| Path | Contents |
|---|---|
| `%PROGRAMFILES%\ProjectPacker\` (MSI) | Binary |
| `%APPDATA%\ProjectPacker\` | `settings.json`, presets, recents |
| `%APPDATA%\ProjectPacker\logs\` | Rolling JSON-line logs |
| `%TEMP%\projectpacker\{job_id}\` | Cloned repos during job (auto-cleanup) |

Uninstaller leaves `%APPDATA%` alone (settings survive reinstall).

### 11.8 First-run experience

Dark theme by default. Empty Recents with centered "New Pack" CTA. No setup wizard, no telemetry consent dialog, no tour. Time-to-first-pack target: <30 seconds on a small folder.

### 11.9 License & repo metadata

- License: **MIT** (recommended).
- Repo: `github.com/CBaileyDev/ProjectPacker`, public.
- README mirrors V1 structure: features, screenshots, install, usage, build, link to design + protocol docs.
- `CHANGELOG.md` from day one.

## 12. Open questions / future work

- Cross-platform builds (macOS / Linux) — Tauri makes these trivial CI additions when ready.
- CLI sibling binary (post-v1).
- Built-in chat panel with API keys — for users who want one-app workflow.
- Auto-update via Tauri updater plugin.
- Code signing.
- Multiple concurrent pack jobs.
- Pack history with diffs against previous packs of the same target.
- Protocol v2: structured "Files Touched" summary table, optional `expected_diff` per step, machine-readable JSON sidecar.
- Plugin system for handoff protocols (Cursor, aider, etc.).
- File associations / `projectpacker://` URI scheme / "Pack with ProjectPacker" Explorer right-click entry.

## 13. Appendix A — Component inventory for Claude Design

The following components are the design surface Claude Design will produce. Each receives a typed props contract from `frontend/src/bindings/`.

| Component | Purpose |
|---|---|
| `<TargetPicker>` | Folder drop / GitHub URL input |
| `<GoalEditor>` | Task description + saved templates |
| `<OptionsPanel>` | Toggles + advanced disclosure + presets dropdown |
| `<PackProgress>` | The live animation panel during pack |
| `<FileTickerStream>` | Scrolling list of files being scanned |
| `<StageIndicator>` | Walking → Tokenizing → Scanning → Building |
| `<PackStatsCard>` | Result header (file count, bytes, tokens, time) |
| `<CopyButton>` | Reusable copy-with-feedback button |
| `<XmlPreview>` | Syntax-highlighted read-only preview |
| `<SecretWarningBanner>` | Shown when secrets > 0 |
| `<RecentsList>` | Home screen card list |
| `<PlanValidator>` | Bridge tab — paste plan, render errors or success |
| `<RePromptBuilder>` | Bridge tab — copy "fix your plan" prompt for Grok |
| `<CombinedPromptCopy>` | Bridge tab — copy wrapped CC prompt with embedded plan |
| `<ToastQueue>` | Bottom-right transient notifications |
| `<ErrorPanel>` | Full-screen fatal error treatment |
| `<WarningsCard>` | Result screen non-fatal warnings list |

## 14. Appendix B — Claude Design prompt (draft, to be sent in Phase 3)

The two `[Paste …]` markers in the prompt below are intentional placeholders. They are filled at send time with (a) the contents of `frontend/src/bindings/index.ts` (auto-generated from Rust types after the first scaffold build) and (b) the component inventory table from §13. Do not pre-fill them in this design doc — they only become concrete once the scaffold exists.

```
Hi! I'm building a Windows desktop app called ProjectPacker — a Tauri +
Rust + React 19 + TypeScript + Vite + Tailwind v4 application. Its job is
to pack a code repository into a single XML file optimized for an AI
planning workflow.

I want you to design the React UI. The Rust backend, Tauri commands, and
TypeScript type bindings already exist. Your job is the visual surface.

## Style direction

Aim for a polished, premium, dev-tool aesthetic — closer to Linear, Vercel,
or Raycast than to a generic Bootstrap app. Dark mode by default. Heavy use
of motion: page transitions, springy hovers, scrolling tickers during long
operations, particle effects on pack completion if it serves the moment.
Tasteful glassmorphism, subtle gradients, and animated borders are welcome.
Use Framer Motion for declarative animation. Use shadcn/ui as the
foundation; layer Aceternity UI / MagicUI where they elevate the moment.

## Routes to design

1. Home — landing screen with a giant "New Pack" CTA and a list of recent
   packs as cards. Animated background. Empty-state treatment when there
   are no recents.
2. Pack — the workspace. Three vertical sections: Target (folder drop or
   GitHub URL), Goal (multiline text area with templates), Options
   (toggles + advanced disclosure + presets dropdown). Bottom bar with a
   single primary "Pack" button that transforms into a live progress
   panel during pack. The progress panel is the visual centerpiece —
   animated stage indicator, file ticker, elapsed timer, cancel button.
3. Result — pack stats header, two big "Copy" buttons side by side
   ("Copy Pack XML" and "Copy Claude Code Prompt"), a "Save as…" button,
   and tabs below for Pack XML / CC Prompt / Warnings / Skipped Files.
4. Bridge — paste Grok's plan into a textarea; on submission, show
   either a cleanly-formatted error list (with a "Copy re-prompt for
   Grok" button) or a success state with a single "Copy combined
   prompt" button. The transition between paste-pending → validating →
   success should feel rewarding.
5. Settings — manage presets, edit ignore defaults, change theme, set
   default tokenizer model, edit goal templates. Functional, tidy,
   secondary in importance.

## Data shapes

[Paste the contents of frontend/src/bindings/index.ts here when sending
the prompt — these are the types the components will receive as props.]

## Components to build

[Paste the Component inventory table from the design doc — each row
becomes a discrete component with its purpose.]

## Constraints

- All components live under frontend/src/components/ and frontend/src/routes/.
- No external network calls — the app is fully offline.
- Components should accept their data via typed props matching the
  bindings/ types; do not invent new shapes.
- Animations should be tasteful, never block input, never run >2s.
- Accessibility: keyboard navigation everywhere, focus rings visible,
  ARIA labels on icon-only buttons.

## What I want back

Code. Drop-in .tsx files for each route and component, with all CSS
inline as Tailwind classes. Use Framer Motion. Use lucide-react for
icons. Match the typed props from the bindings exactly.

When you have a draft, show me the Pack route first — that is the visual
centerpiece of the app.
```

---

## Approval

This document is the authoritative design for ProjectPacker v1.0. Implementation does not begin until this doc is approved by the user. The next step after approval is invoking the writing-plans skill to produce a detailed step-by-step implementation plan in `docs/superpowers/plans/`.
