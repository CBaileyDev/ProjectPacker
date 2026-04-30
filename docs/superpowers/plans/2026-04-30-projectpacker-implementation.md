# ProjectPacker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build ProjectPacker v0.1.0 — a Windows Tauri desktop app (Rust core + React UI) that packs a folder or GitHub repo into a self-describing XML file optimized for a Grok → Claude Code two-AI workflow.

**Architecture:** Tauri 2 app with a workspace of two Rust crates (`core` library + `app` Tauri shell), a React 19 + TS frontend, and `specta` auto-generated TypeScript bindings. The Rust core does all heavy work; the UI is a deliberately-minimal placeholder that will be replaced by components from Claude Design in a later phase.

**Tech Stack:** Rust (tokio, ignore, gix, tiktoken-rs, tree-sitter, quick-xml, regex, rayon, thiserror, serde) · Tauri 2 · specta + tauri-specta · React 19 + TypeScript 5 + Vite 6 + Tailwind 4 · Zustand · vitest · Playwright · GitHub Actions.

**Reference spec:** `docs/superpowers/specs/2026-04-30-projectpacker-design.md`

---

## How to use this plan

- **Working directory** for every task is the project root (`ProjectPacker/` after Phase 0; `V2/` before).
- Every task ends with a commit. Frequent commits = recoverable state.
- Tests come before implementation (TDD). When a step says "Run test, expect FAIL," that failure is part of the contract — do not skip it.
- Versions in code blocks reflect what was current at plan-write time; let `cargo update` and `pnpm up` do their job during scaffolding, but pin to known-good versions in the lockfiles.
- The plan stops short of building polished UI components — those come from Claude Design (see Phase 7). Phase 5's UI is intentionally ugly placeholder plumbing.

---

# Phase 0 — Project bootstrap

## Task 0.1: Rename V2 → ProjectPacker; create directory structure

**Files:**
- Rename: `e:/Tools/Parsers/V2/` → `e:/Tools/Parsers/ProjectPacker/`
- Create directories per spec §5.3.

- [ ] **Step 1: Rename the folder**

```bash
cd /e/Tools/Parsers
mv V2 ProjectPacker
cd ProjectPacker
```

Verify the design doc and plan are still present:

```bash
ls docs/superpowers/specs docs/superpowers/plans
```

Expected: each lists the corresponding 2026-04-30 markdown file.

- [ ] **Step 2: Create the directory skeleton**

```bash
mkdir -p crates/core/src crates/app/src crates/app/icons
mkdir -p frontend/src/{bindings,lib,components,routes,styles}
mkdir -p docs/protocol
mkdir -p tests/fixtures/{tiny,medium,binary} tests/e2e
mkdir -p scripts .github/workflows
```

- [ ] **Step 3: Verify the layout matches §5.3**

```bash
find . -type d -not -path './docs/*' -not -path '*/node_modules/*' | sort
```

Expected output should contain (among others): `./crates/app/icons`, `./crates/app/src`, `./crates/core/src`, `./frontend/src/bindings`, `./frontend/src/components`, `./frontend/src/lib`, `./frontend/src/routes`, `./tests/fixtures/binary`, `./tests/fixtures/medium`, `./tests/fixtures/tiny`.

- [ ] **Step 4: Commit (deferred — first commit happens after Task 0.4 once we have actual content).**

---

## Task 0.2: Initialize Rust workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/app/Cargo.toml`
- Create: `crates/app/src/main.rs`
- Create: `crates/app/build.rs`
- Create: `rust-toolchain.toml`

- [ ] **Step 1: Pin the toolchain**

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.85.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

- [ ] **Step 2: Create the workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/app"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["CBaileyDev"]
repository = "https://github.com/CBaileyDev/ProjectPacker"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "sync", "time"] }
specta = { version = "=2.0.0-rc.22", features = ["derive"] }
specta-typescript = "=0.0.9"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

[profile.release]
strip = true
lto = "thin"
codegen-units = 1
```

- [ ] **Step 3: Create `crates/core/Cargo.toml`**

```toml
[package]
name = "projectpacker-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true
tokio.workspace = true
specta.workspace = true

ignore = "0.4"
gix = { version = "0.66", default-features = false, features = ["blocking-network-client", "blocking-http-transport-reqwest", "max-performance-safe"] }
tiktoken-rs = "0.6"
tree-sitter = "0.25"
quick-xml = { version = "0.37", features = ["serialize"] }
regex = "1"
rayon = "1.10"
sha2 = "0.10"
uuid = { version = "1", features = ["v7"] }
encoding_rs = "0.8"
walkdir = "2"

[dev-dependencies]
insta = { version = "1", features = ["yaml"] }
proptest = "1"
tempfile = "3"
tokio = { workspace = true, features = ["rt", "macros"] }
```

- [ ] **Step 4: Create `crates/core/src/lib.rs`**

```rust
//! ProjectPacker core library — pure Rust packing pipeline. No Tauri deps.

pub mod error;
pub mod ignore;
pub mod protocol;
pub mod types;
pub mod walker;
```

The other modules (`pack`, `secrets`, `tokens`, `tree_sitter`, `github`) will be added in later tasks. For now we expose only the four modules created in Phase 1.

- [ ] **Step 5: Create `crates/app/Cargo.toml`**

```toml
[package]
name = "projectpacker-app"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
default-run = "projectpacker-app"

[lib]
name = "projectpacker_app_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
projectpacker-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
specta.workspace = true
specta-typescript.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

tauri = { version = "2", features = [] }
tauri-specta = { version = "=2.0.0-rc.21", features = ["derive", "typescript"] }
tauri-plugin-dialog = "2"
tauri-plugin-clipboard-manager = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"
arboard = "3"
tracing-appender = "0.2"
dashmap = "6"
parking_lot = "0.12"
```

- [ ] **Step 6: Create `crates/app/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 7: Create a placeholder `crates/app/src/main.rs`**

```rust
fn main() {
    projectpacker_app_lib::run();
}
```

- [ ] **Step 8: Create `crates/app/src/lib.rs` (placeholder, expanded in Phase 4)**

```rust
//! ProjectPacker Tauri shell. Will own commands, events, settings.

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 9: Verify the workspace compiles**

```bash
cargo check --workspace
```

Expected: `Checking projectpacker-core …` then `Checking projectpacker-app …` then `Finished`. (Tauri will complain about a missing `tauri.conf.json` and frontend dist — that's expected and fixed in Task 0.3 / 0.4.)

If you see an unrelated build error, fix it before continuing.

- [ ] **Step 10: Commit (deferred — see Task 0.4).**

---

## Task 0.3: Initialize the React frontend

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/tsconfig.node.json`
- Create: `frontend/tailwind.config.ts`
- Create: `frontend/postcss.config.js`
- Create: `frontend/index.html`
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/styles/globals.css`

- [ ] **Step 1: Create `frontend/package.json`**

```json
{
  "name": "projectpacker-frontend",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "typecheck": "tsc -b --noEmit",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-clipboard-manager": "^2",
    "@tauri-apps/plugin-fs": "^2",
    "@tauri-apps/plugin-shell": "^2",
    "react": "^19",
    "react-dom": "^19",
    "react-router-dom": "^7",
    "zustand": "^5"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@testing-library/react": "^16",
    "@types/react": "^19",
    "@types/react-dom": "^19",
    "@vitejs/plugin-react": "^4",
    "happy-dom": "^15",
    "tailwindcss": "^4",
    "@tailwindcss/postcss": "^4",
    "typescript": "^5.6",
    "vite": "^6",
    "vitest": "^3"
  }
}
```

- [ ] **Step 2: Create `frontend/vite.config.ts`**

```ts
/// <reference types="vitest" />
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    hmr: { protocol: "ws", host: "localhost", port: 1421 },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "chrome120",
    minify: "esbuild",
    sourcemap: true,
    outDir: "dist",
  },
  test: {
    environment: "happy-dom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
```

- [ ] **Step 3: Create `frontend/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 4: Create `frontend/tsconfig.node.json`**

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 5: Create `frontend/tailwind.config.ts`**

```ts
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: { extend: {} },
} satisfies Config;
```

- [ ] **Step 6: Create `frontend/postcss.config.js`**

```js
export default {
  plugins: {
    "@tailwindcss/postcss": {},
  },
};
```

- [ ] **Step 7: Create `frontend/index.html`**

```html
<!DOCTYPE html>
<html lang="en" class="dark">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>ProjectPacker</title>
  </head>
  <body class="bg-zinc-950 text-zinc-100">
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 8: Create `frontend/src/styles/globals.css`**

```css
@import "tailwindcss";

:root {
  color-scheme: dark;
}

html, body, #root {
  height: 100%;
  margin: 0;
}
```

- [ ] **Step 9: Create `frontend/src/main.tsx`**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 10: Create `frontend/src/App.tsx` (placeholder)**

```tsx
export default function App() {
  return (
    <div className="flex h-full items-center justify-center">
      <div className="text-center">
        <h1 className="text-4xl font-semibold">ProjectPacker</h1>
        <p className="mt-2 text-zinc-400">Scaffold v0.1.0</p>
      </div>
    </div>
  );
}
```

- [ ] **Step 11: Install frontend dependencies**

```bash
cd frontend
pnpm install
cd ..
```

If `pnpm` is not installed: `npm i -g pnpm` first. If `pnpm install` fails with peer-dep complaints, do NOT add `--shamefully-hoist`; instead read the error and add the missing peer to package.json.

- [ ] **Step 12: Verify the frontend builds**

```bash
cd frontend
pnpm build
cd ..
```

Expected: `vite v6.x.x building for production... ✓ built in …ms`. The output is in `frontend/dist/`.

- [ ] **Step 13: Commit (deferred to Task 0.4).**

---

## Task 0.4: Configure Tauri shell, init git, push to GitHub

**Files:**
- Create: `crates/app/tauri.conf.json`
- Create: `crates/app/icons/icon.png` (placeholder, replaced later)
- Create: `crates/app/Tauri.toml` (none — using JSON)
- Create: `.gitignore`
- Create: `.gitattributes`
- Create: `README.md`
- Create: `LICENSE`
- Create: `CHANGELOG.md`
- Modify: workspace `Cargo.toml` (already created)

- [ ] **Step 1: Create `crates/app/tauri.conf.json`**

```json
{
  "$schema": "../../node_modules/@tauri-apps/cli/config.schema.json",
  "productName": "ProjectPacker",
  "version": "0.1.0",
  "identifier": "dev.cbailey.projectpacker",
  "build": {
    "frontendDist": "../../frontend/dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "pnpm --filter projectpacker-frontend dev",
    "beforeBuildCommand": "pnpm --filter projectpacker-frontend build"
  },
  "app": {
    "windows": [
      {
        "title": "ProjectPacker",
        "width": 1280,
        "height": 800,
        "minWidth": 960,
        "minHeight": 600,
        "decorations": true,
        "transparent": false,
        "resizable": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis"],
    "icon": ["icons/icon.png"],
    "category": "DeveloperTool",
    "shortDescription": "Pack a repo into XML for AI workflows",
    "longDescription": "ProjectPacker turns a folder or GitHub repo into a single self-describing XML file optimized for a Grok → Claude Code two-AI workflow.",
    "windows": {
      "wix": { "language": "en-US" },
      "nsis": { "installMode": "currentUser" }
    }
  }
}
```

- [ ] **Step 2: Create placeholder icon files**

Tauri 2's `tauri-build` always generates a Windows resource file using `icon.ico`, *even with `--no-bundle`*. So both PNG and ICO are required. From PowerShell:

```powershell
# Tiny placeholder PNG (4×4)
$pngBytes = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAFklEQVR42mNk+M9Qz0AEYBxVSF+FAEy3AYDpkxrnAAAAAElFTkSuQmCC')
[IO.File]::WriteAllBytes("crates/app/icons/icon.png", $pngBytes)

# Minimal 1×1 ICO (76 bytes) — required by tauri-build's Windows resource step
$icoBytes = [Convert]::FromBase64String('AAABAAEAAQEAAAEAGAAwAAAAFgAAACgAAAABAAAAAgAAAAEAGAAAAAAACAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==')
[IO.File]::WriteAllBytes("crates/app/icons/icon.ico", $icoBytes)
```

A real icon set comes later. The bundler just needs valid PNG + ICO files.

- [ ] **Step 3: Create the workspace `pnpm-workspace.yaml`** (lets pnpm find the frontend)

```yaml
packages:
  - "frontend"
```

- [ ] **Step 4: Create `.gitignore`**

```gitignore
# Rust
target/
Cargo.lock.bak

# Node
node_modules/
.pnpm-store/

# Builds
dist/
build/
*.exe
*.msi

# tsc -b emits these next to vite.config.ts when composite: true
frontend/vite.config.js
frontend/vite.config.d.ts
frontend/*.tsbuildinfo

# Generated bindings (regenerated each build)
frontend/src/bindings/

# IDE
.vscode/
.idea/

# OS
Thumbs.db
.DS_Store

# Tauri
crates/app/gen/
.tauri/

# Logs
*.log

# Local env
.env
.env.local
```

- [ ] **Step 5: Create `.gitattributes`**

```gitattributes
* text=auto eol=lf
*.bat text eol=crlf
*.ps1 text eol=crlf
*.png binary
*.ico binary
*.exe binary
*.msi binary
```

- [ ] **Step 6: Create `LICENSE`** (MIT)

```
MIT License

Copyright (c) 2026 CBaileyDev

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

- [ ] **Step 7: Create `README.md`**

```markdown
# ProjectPacker

Windows desktop app that packs a folder or GitHub repository into a single self-describing XML file optimized for a two-AI workflow:

1. **Plan with Grok** — paste the pack into Grok (web, multi-agent). Grok produces a strict-format change plan with rationale per step.
2. **Execute with Claude Code** — paste the plan into Claude Code running in the target repo. Claude Code reviews, may challenge any step, then executes.

ProjectPacker itself never edits files — it is a packer, a protocol layer, and a validator.

## Status

v0.1.0 — early scaffold. Not yet released.

## Stack

- Tauri 2 desktop shell
- Rust core (file walker, ignore, secrets, tokens, tree-sitter, XML, protocol)
- React 19 + TypeScript + Vite + Tailwind 4 frontend
- specta-generated TypeScript bindings

## Documentation

- [Design doc](docs/superpowers/specs/2026-04-30-projectpacker-design.md)
- [Implementation plan](docs/superpowers/plans/2026-04-30-projectpacker-implementation.md)
- [Protocol: grok-to-cc-v1](docs/protocol/grok-to-cc-v1.md)

## Development

```bash
pnpm install
pnpm tauri dev
```

## License

MIT — see [LICENSE](LICENSE).
```

- [ ] **Step 8: Create `CHANGELOG.md`**

```markdown
# Changelog

All notable changes to ProjectPacker are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial workspace scaffold (Rust workspace + Tauri shell + React/Vite/Tailwind frontend).
- Design doc and implementation plan committed.
- MIT license.
```

- [ ] **Step 9: Verify the full app builds**

```bash
pnpm install
pnpm tauri build --debug --no-bundle
```

Expected: Tauri compiles `projectpacker-app`, builds the frontend, and links them. The first build will be slow (~5-10 minutes); subsequent builds incremental.

If this fails, do NOT proceed. Fix the build before continuing — every later task assumes a working baseline.

- [ ] **Step 10: Initialize git and create the first commit**

```bash
git init -b main
git add -A
git status
```

Review the staged files. Confirm `frontend/node_modules/`, `target/`, `frontend/dist/`, and `crates/app/gen/` are NOT staged. If any are, fix `.gitignore` and re-run.

```bash
git commit -m "$(cat <<'EOF'
Initial scaffold: Tauri 2 + Rust workspace + React/Vite/Tailwind frontend

- Workspace: crates/core (library) + crates/app (Tauri shell)
- Frontend: React 19 + TypeScript + Vite 6 + Tailwind 4
- Design doc + implementation plan committed
- MIT license; Windows-only target for v0.1.0
EOF
)"
```

- [ ] **Step 11: Add the GitHub remote and prepare to push**

```bash
git remote add origin https://github.com/CBaileyDev/ProjectPacker.git
git remote -v
```

Expected: `origin  https://github.com/CBaileyDev/ProjectPacker.git (fetch)` and `(push)`.

**The actual `git push -u origin main` is a manual step the user runs** — they have the credentials. Print this exact command for the user:

> `git push -u origin main`

Stop and confirm with the user that the push succeeded before continuing to Phase 1.

---

# Phase 1 — Rust core data layer

## Task 1.1: `core::types` — shared data shapes

**Files:**
- Create: `crates/core/src/types.rs`
- Create: `crates/core/src/types/tests.rs` (inline `#[cfg(test)]` instead — single file)

- [ ] **Step 1: Write the test for serde round-tripping**

Create `crates/core/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "value")]
pub enum PackTarget {
    #[serde(rename = "folder")]
    Folder(PathBuf),
    #[serde(rename = "github")]
    GitHub(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackOptions {
    pub target: PackTarget,
    pub goal: String,
    pub include_git_history: bool,
    pub count_tokens: bool,
    pub tokenizer_model: String,
    pub secret_scan: bool,
    pub compress: bool,
    pub remove_comments: bool,
    pub max_file_size_kb: u32,
    pub respect_gitignore: bool,
    pub custom_ignore_patterns: Vec<String>,
    pub protocol_version: String,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            target: PackTarget::Folder(PathBuf::from(".")),
            goal: String::new(),
            include_git_history: false,
            count_tokens: true,
            tokenizer_model: "gpt-4o-mini".into(),
            secret_scan: true,
            compress: false,
            remove_comments: false,
            max_file_size_kb: 1024,
            respect_gitignore: true,
            custom_ignore_patterns: Vec::new(),
            protocol_version: "grok-to-cc-v1".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackStats {
    pub files_total: u32,
    pub files_included: u32,
    pub files_skipped: u32,
    pub bytes_total: u64,
    pub tokens_total: Option<u32>,
    pub secrets_found: u32,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkipReason {
    Ignored,
    TooLarge,
    Binary,
    Inaccessible,
    EncodingFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum WarningKind {
    FileSkipped,
    TreeSitterFailed,
    GitLogMissing,
    EncodingFallback,
    SecretScanFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackWarning {
    pub kind: WarningKind,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileFound {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ProgressEvent {
    Started { job_id: String, target_label: String },
    Cloning { progress_pct: u8 },
    Walking { files_scanned: u32 },
    FileFoundBatch { paths: Vec<FileFound> },
    FileSkipped { path: String, reason: SkipReason },
    Tokenizing { progress_pct: u8 },
    SecretScanning { progress_pct: u8 },
    SecretHit { path: String, secret_kind: String, line: u32 },
    Compressing { progress_pct: u8 },
    BuildingXml,
    Done { stats: PackStats },
    Error { message: String, fatal: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackResult {
    pub xml: String,
    pub claude_code_prompt: String,
    pub stats: PackStats,
    pub warnings: Vec<PackWarning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_options_default_has_v1_protocol() {
        let opts = PackOptions::default();
        assert_eq!(opts.protocol_version, "grok-to-cc-v1");
        assert_eq!(opts.tokenizer_model, "gpt-4o-mini");
        assert_eq!(opts.max_file_size_kb, 1024);
        assert!(opts.respect_gitignore);
    }

    #[test]
    fn pack_target_round_trips_through_json_folder() {
        let t = PackTarget::Folder(PathBuf::from("/tmp/repo"));
        let s = serde_json::to_string(&t).unwrap();
        let back: PackTarget = serde_json::from_str(&s).unwrap();
        match back {
            PackTarget::Folder(p) => assert_eq!(p, PathBuf::from("/tmp/repo")),
            _ => panic!("expected Folder variant"),
        }
    }

    #[test]
    fn pack_target_round_trips_through_json_github() {
        let t = PackTarget::GitHub("https://github.com/user/repo".into());
        let s = serde_json::to_string(&t).unwrap();
        let back: PackTarget = serde_json::from_str(&s).unwrap();
        match back {
            PackTarget::GitHub(u) => assert_eq!(u, "https://github.com/user/repo"),
            _ => panic!("expected GitHub variant"),
        }
    }

    #[test]
    fn progress_event_done_serializes_with_stats() {
        let ev = ProgressEvent::Done {
            stats: PackStats {
                files_total: 10,
                files_included: 9,
                files_skipped: 1,
                bytes_total: 12345,
                tokens_total: Some(2000),
                secrets_found: 0,
                duration_ms: 200,
            },
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains("\"kind\":\"done\""));
        assert!(s.contains("\"filesTotal\":10"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail (file doesn't compile yet — `error` module is missing)**

```bash
cargo test -p projectpacker-core types::tests
```

Expected: compile errors mentioning the missing `error` module from `lib.rs`. **This is normal** — we declared `pub mod error;` in lib.rs but haven't created it yet.

Temporarily comment out the unused `pub mod` lines in `crates/core/src/lib.rs`:

```rust
//! ProjectPacker core library.

pub mod types;
// pub mod error; — added in Task 1.2
// pub mod ignore; — added in Task 1.3
// pub mod walker; — added in Task 1.4
// pub mod protocol; — added in Phase 3
```

- [ ] **Step 3: Re-run the tests; expect PASS**

```bash
cargo test -p projectpacker-core types::tests
```

Expected: `test result: ok. 4 passed`.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/types.rs crates/core/src/lib.rs
git commit -m "feat(core): add shared types module with PackOptions/PackResult/ProgressEvent"
```

---

## Task 1.2: `core::error` — error types

**Files:**
- Create: `crates/core/src/error.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the test**

Append to a new file `crates/core/src/error.rs`:

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("invalid target: {0}")]
    InvalidTarget(String),

    #[error("path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("github clone failed: {0}")]
    CloneFailed(String),

    #[error("file walk failed: {0}")]
    WalkFailed(String),

    #[error("io error reading {path}: {source}")]
    FileIo { path: PathBuf, #[source] source: std::io::Error },

    #[error("xml emission failed: {0}")]
    XmlWrite(String),

    #[error("tokenizer not available for model: {0}")]
    TokenizerUnavailable(String),

    #[error("plan validation failed: {errors:?}")]
    PlanInvalid { errors: Vec<String> },

    #[error("cancelled by user")]
    Cancelled,

    #[error("internal: {0}")]
    Internal(String),
}

pub type CoreResult<T> = Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_target_displays_as_expected() {
        let e = CoreError::InvalidTarget("not a url".into());
        assert_eq!(e.to_string(), "invalid target: not a url");
    }

    #[test]
    fn cancelled_has_no_args() {
        let e = CoreError::Cancelled;
        assert_eq!(e.to_string(), "cancelled by user");
    }

    #[test]
    fn plan_invalid_includes_errors() {
        let e = CoreError::PlanInvalid { errors: vec!["missing Summary".into(), "no rationale on Step 2".into()] };
        let s = e.to_string();
        assert!(s.contains("missing Summary"));
        assert!(s.contains("no rationale on Step 2"));
    }
}
```

- [ ] **Step 2: Re-enable the `error` module in `lib.rs`**

```rust
pub mod types;
pub mod error;
// pub mod ignore; — added in Task 1.3
// pub mod walker; — added in Task 1.4
// pub mod protocol; — added in Phase 3
```

- [ ] **Step 3: Run the tests, expect PASS**

```bash
cargo test -p projectpacker-core error::tests
```

Expected: `test result: ok. 3 passed`.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/error.rs crates/core/src/lib.rs
git commit -m "feat(core): add CoreError enum with thiserror"
```

---

## Task 1.3: `core::ignore` — ignore matcher

**Files:**
- Create: `crates/core/src/ignore.rs`
- Create: `crates/core/src/ignore_defaults.txt`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Create the built-in defaults file**

Create `crates/core/src/ignore_defaults.txt`:

```
# Built-in ProjectPacker ignore defaults — combined with .gitignore + .codeparserignore

# Version control
.git/
.svn/
.hg/

# Dependencies
node_modules/
bower_components/
vendor/
.venv/
venv/
__pycache__/
*.pyc
.pnpm-store/

# Build outputs
target/
build/
dist/
out/
.next/
.nuxt/
.parcel-cache/
.turbo/
.vite/
.tauri/

# IDE / editor
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
desktop.ini

# Lockfiles (still text but rarely useful in a pack)
package-lock.json
pnpm-lock.yaml
yarn.lock
Cargo.lock
poetry.lock
Gemfile.lock
go.sum

# Binary / large media
*.png
*.jpg
*.jpeg
*.gif
*.bmp
*.ico
*.icns
*.webp
*.svg
*.mp3
*.mp4
*.mov
*.avi
*.zip
*.tar
*.tar.gz
*.tgz
*.7z
*.rar
*.exe
*.dll
*.so
*.dylib
*.a
*.lib
*.pdf
*.woff
*.woff2
*.ttf
*.otf
*.eot

# Logs
*.log
logs/
```

- [ ] **Step 2: Write the test, then implementation**

Create `crates/core/src/ignore.rs`:

```rust
use ::ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::Path;

const BUILTIN_DEFAULTS: &str = include_str!("ignore_defaults.txt");

pub struct IgnoreMatcher {
    builtin: Gitignore,
    project: Option<Gitignore>,
    custom: Option<Gitignore>,
}

impl IgnoreMatcher {
    pub fn new(
        project_root: &Path,
        custom_patterns: &[String],
        respect_gitignore: bool,
    ) -> Self {
        let builtin = build_from_lines(BUILTIN_DEFAULTS.lines(), Path::new(""));

        let project = if respect_gitignore {
            Some(build_project(project_root))
        } else {
            None
        };

        let custom = if custom_patterns.is_empty() {
            None
        } else {
            Some(build_from_lines(custom_patterns.iter().map(String::as_str), Path::new("")))
        };

        Self { builtin, project, custom }
    }

    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        // matched_path_or_any_parents climbs parents — needed because
        // matched() alone doesn't fire on "node_modules/foo.js" when the
        // pattern is "node_modules/".
        let m = self.builtin.matched_path_or_any_parents(path, is_dir);
        if m.is_ignore() { return true; }

        if let Some(p) = &self.project {
            let m = p.matched_path_or_any_parents(path, is_dir);
            if m.is_ignore() { return true; }
            if m.is_whitelist() { return false; }
        }

        if let Some(c) = &self.custom {
            let m = c.matched_path_or_any_parents(path, is_dir);
            if m.is_ignore() { return true; }
        }

        false
    }
}

fn build_from_lines<'a>(lines: impl IntoIterator<Item = &'a str>, root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let _ = b.add_line(None, line);
    }
    b.build().expect("ignore: builtin pattern compile failure")
}

fn build_project(root: &Path) -> Gitignore {
    let mut b = GitignoreBuilder::new(root);
    let _ = b.add(root.join(".gitignore"));
    let _ = b.add(root.join(".codeparserignore"));
    b.build().unwrap_or_else(|_| Gitignore::empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn root(p: &Path) -> IgnoreMatcher {
        IgnoreMatcher::new(p, &[], true)
    }

    #[test]
    fn builtin_ignores_node_modules() {
        let m = root(Path::new("/tmp/empty"));
        assert!(m.is_ignored(Path::new("node_modules/foo.js"), false));
        assert!(m.is_ignored(Path::new("node_modules"), true));
    }

    #[test]
    fn builtin_ignores_lockfiles() {
        let m = root(Path::new("/tmp/empty"));
        assert!(m.is_ignored(Path::new("package-lock.json"), false));
        assert!(m.is_ignored(Path::new("Cargo.lock"), false));
        assert!(m.is_ignored(Path::new("pnpm-lock.yaml"), false));
    }

    #[test]
    fn does_not_ignore_arbitrary_source_files() {
        let m = root(Path::new("/tmp/empty"));
        assert!(!m.is_ignored(Path::new("src/main.rs"), false));
        assert!(!m.is_ignored(Path::new("README.md"), false));
    }

    #[test]
    fn project_gitignore_takes_effect() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "secret/\n*.bak\n").unwrap();
        let m = root(dir.path());
        assert!(m.is_ignored(Path::new("secret/x.txt"), false));
        assert!(m.is_ignored(Path::new("foo.bak"), false));
        assert!(!m.is_ignored(Path::new("foo.txt"), false));
    }

    #[test]
    fn custom_patterns_layer_on_top() {
        let m = IgnoreMatcher::new(Path::new("/tmp/empty"), &["docs/private/".into()], false);
        assert!(m.is_ignored(Path::new("docs/private/secret.md"), false));
    }

    #[test]
    fn respect_gitignore_false_disables_project_rules() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "src/\n").unwrap();
        let m = IgnoreMatcher::new(dir.path(), &[], false);
        assert!(!m.is_ignored(Path::new("src/main.rs"), false));
    }

    #[test]
    fn _unused_pathbuf_to_silence_warning() {
        let _ = PathBuf::new();
    }
}
```

- [ ] **Step 3: Re-enable the module in `lib.rs`**

```rust
pub mod types;
pub mod error;
pub mod ignore;
// pub mod walker; — added in Task 1.4
// pub mod protocol; — added in Phase 3
```

- [ ] **Step 4: Run the tests; expect PASS**

```bash
cargo test -p projectpacker-core ignore::tests
```

Expected: `test result: ok. 7 passed` (the dummy test is included).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/ignore.rs crates/core/src/ignore_defaults.txt crates/core/src/lib.rs
git commit -m "feat(core): add IgnoreMatcher with builtin + project + custom layers"
```

---

## Task 1.4: `core::walker` — async file walker

**Files:**
- Create: `crates/core/src/walker.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the test + implementation**

Create `crates/core/src/walker.rs`:

```rust
use crate::ignore::IgnoreMatcher;
use crate::types::{FileFound, SkipReason};
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct WalkOutcome {
    pub included: Vec<FileFound>,
    pub skipped: Vec<(String, SkipReason)>,
}

pub struct WalkOptions {
    pub max_file_size_kb: u32,
}

pub fn walk(root: &Path, matcher: &IgnoreMatcher, opts: &WalkOptions) -> WalkOutcome {
    let mut included = Vec::new();
    let mut skipped = Vec::new();

    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() { continue; }

        let abs = entry.path();
        let rel = match abs.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if matcher.is_ignored(rel, false) {
            skipped.push((rel_str, SkipReason::Ignored));
            continue;
        }

        let bytes = match entry.metadata() {
            Ok(m) => m.len(),
            Err(_) => {
                skipped.push((rel_str, SkipReason::Inaccessible));
                continue;
            }
        };

        if bytes > (opts.max_file_size_kb as u64) * 1024 {
            skipped.push((rel_str, SkipReason::TooLarge));
            continue;
        }

        if is_binary(abs) {
            skipped.push((rel_str, SkipReason::Binary));
            continue;
        }

        included.push(FileFound { path: rel_str, bytes });
    }

    WalkOutcome { included, skipped }
}

fn is_binary(path: &Path) -> bool {
    let mut buf = [0u8; 8192];
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open(path) {
        if let Ok(n) = f.read(&mut buf) {
            return buf[..n].contains(&0u8);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_fixture() -> tempfile::TempDir {
        let d = tempdir().unwrap();
        fs::write(d.path().join("a.txt"), "hello\n").unwrap();
        fs::write(d.path().join("b.rs"), "fn main() {}\n").unwrap();
        fs::create_dir(d.path().join("node_modules")).unwrap();
        fs::write(d.path().join("node_modules/x.js"), "noop").unwrap();
        fs::write(d.path().join("big.txt"), vec![b'x'; 4096]).unwrap();
        fs::write(d.path().join("binary.bin"), vec![0u8, 1, 2, 3]).unwrap();
        d
    }

    #[test]
    fn walks_and_skips_node_modules() {
        let d = make_fixture();
        let m = IgnoreMatcher::new(d.path(), &[], false);
        let out = walk(d.path(), &m, &WalkOptions { max_file_size_kb: 1024 });
        let included: Vec<_> = out.included.iter().map(|f| f.path.as_str()).collect();
        assert!(included.contains(&"a.txt"));
        assert!(included.contains(&"b.rs"));
        assert!(!included.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn skips_oversize_files() {
        let d = make_fixture();
        let m = IgnoreMatcher::new(d.path(), &[], false);
        let out = walk(d.path(), &m, &WalkOptions { max_file_size_kb: 1 });
        let big_skipped = out.skipped.iter().any(|(p, r)| p == "big.txt" && matches!(r, SkipReason::TooLarge));
        assert!(big_skipped, "big.txt should be skipped as TooLarge");
    }

    #[test]
    fn skips_binary_files() {
        let d = make_fixture();
        let m = IgnoreMatcher::new(d.path(), &[], false);
        let out = walk(d.path(), &m, &WalkOptions { max_file_size_kb: 1024 });
        let bin_skipped = out.skipped.iter().any(|(p, r)| p == "binary.bin" && matches!(r, SkipReason::Binary));
        assert!(bin_skipped, "binary.bin should be skipped as Binary");
    }
}
```

- [ ] **Step 2: Re-enable the module in `lib.rs`**

```rust
pub mod types;
pub mod error;
pub mod ignore;
pub mod walker;
// pub mod protocol; — added in Phase 3
```

- [ ] **Step 3: Run the tests; expect PASS**

```bash
cargo test -p projectpacker-core walker::tests
```

Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/walker.rs crates/core/src/lib.rs
git commit -m "feat(core): add synchronous file walker with skip-reason classification"
```

> **Note:** The spec described an async stream walker. We're starting with a synchronous implementation that returns a complete `WalkOutcome`; pack-level streaming/throttling lives in the orchestrator (Task 3.5). This keeps the walker simple and unit-testable. If profiling later shows the synchronous walk is a bottleneck on huge repos, revisit.

---

# Phase 2 — Per-file processors

## Task 2.1: `core::tokens` — tiktoken counter

**Files:**
- Create: `crates/core/src/tokens.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod tokens;`)

- [ ] **Step 1: Write the test + implementation**

Create `crates/core/src/tokens.rs`:

```rust
use crate::error::{CoreError, CoreResult};
use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

static GPT4O_MINI: OnceLock<CoreBPE> = OnceLock::new();

pub fn count(model: &str, text: &str) -> CoreResult<u32> {
    let enc = encoder(model)?;
    Ok(enc.encode_with_special_tokens(text).len() as u32)
}

fn encoder(model: &str) -> CoreResult<&'static CoreBPE> {
    match model {
        "gpt-4o-mini" | "gpt-4o" | "gpt-4" => {
            Ok(GPT4O_MINI.get_or_init(|| {
                tiktoken_rs::o200k_base().expect("o200k_base encoder must initialize")
            }))
        }
        _ => Err(CoreError::TokenizerUnavailable(model.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_tokens_in_simple_string() {
        let n = count("gpt-4o-mini", "Hello, world!").unwrap();
        assert!(n >= 1 && n < 10, "got {n} tokens");
    }

    #[test]
    fn empty_string_is_zero_tokens() {
        let n = count("gpt-4o-mini", "").unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn count_is_deterministic_across_calls() {
        let a = count("gpt-4o-mini", "fn main() { println!(\"hi\") }").unwrap();
        let b = count("gpt-4o-mini", "fn main() { println!(\"hi\") }").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn unknown_model_errors() {
        let err = count("not-a-real-model", "hi").unwrap_err();
        assert!(matches!(err, CoreError::TokenizerUnavailable(_)));
    }
}
```

- [ ] **Step 2: Add `pub mod tokens;` to `lib.rs`**

- [ ] **Step 3: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core tokens::tests
```

Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/tokens.rs crates/core/src/lib.rs
git commit -m "feat(core): add tiktoken-based token counter"
```

---

## Task 2.2: `core::secrets` — secret scanner

**Files:**
- Create: `crates/core/src/secrets.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the test + implementation**

Create `crates/core/src/secrets.rs`:

```rust
use regex::Regex;
use serde::Serialize;
use specta::Type;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretHit {
    pub kind: String,
    pub line: u32,
    pub matched_excerpt: String,
}

struct Rule {
    name: &'static str,
    pattern: Regex,
}

static RULES: OnceLock<Vec<Rule>> = OnceLock::new();

fn rules() -> &'static [Rule] {
    RULES.get_or_init(|| {
        vec![
            Rule { name: "aws-access-key", pattern: Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap() },
            Rule { name: "aws-secret-key", pattern: Regex::new(r#"(?i)aws(.{0,20})?(secret|access)?(.{0,20})?[=:][\s"']*[A-Za-z0-9/+=]{40}"#).unwrap() },
            Rule { name: "github-token", pattern: Regex::new(r"\bghp_[A-Za-z0-9]{36,}\b").unwrap() },
            Rule { name: "github-fine-grained-token", pattern: Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{82}\b").unwrap() },
            Rule { name: "openai-key", pattern: Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_\-]{20,}\b").unwrap() },
            Rule { name: "anthropic-key", pattern: Regex::new(r"\bsk-ant-(?:api03-)?[A-Za-z0-9_\-]{20,}\b").unwrap() },
            Rule { name: "slack-token", pattern: Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{10,}\b").unwrap() },
            Rule { name: "private-key-pem", pattern: Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY-----").unwrap() },
            Rule { name: "google-api-key", pattern: Regex::new(r"\bAIza[0-9A-Za-z_\-]{35}\b").unwrap() },
            Rule { name: "stripe-live-key", pattern: Regex::new(r"\bsk_live_[0-9a-zA-Z]{24,}\b").unwrap() },
        ]
    }).as_slice()
}

pub fn scan(content: &str) -> Vec<SecretHit> {
    let mut hits = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        for rule in rules() {
            if let Some(m) = rule.pattern.find(line) {
                hits.push(SecretHit {
                    kind: rule.name.to_string(),
                    line: (line_idx + 1) as u32,
                    matched_excerpt: redact_excerpt(m.as_str()),
                });
            }
        }
    }
    hits
}

fn redact_excerpt(s: &str) -> String {
    if s.len() <= 8 { return "***".to_string(); }
    let head: String = s.chars().take(4).collect();
    let tail: String = s.chars().rev().take(4).collect::<String>().chars().rev().collect();
    format!("{head}***{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        let hits = scan("token = AKIAIOSFODNN7EXAMPLE\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "aws-access-key");
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn detects_github_token() {
        let hits = scan("ghp_1234567890abcdefghijklmnopqrstuvwxyz1\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "github-token");
    }

    #[test]
    fn detects_pem_private_key() {
        let hits = scan("first line\n-----BEGIN RSA PRIVATE KEY-----\nblah\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "private-key-pem");
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    fn no_false_positive_on_innocuous_string() {
        let hits = scan("let x = \"hello world\";\nfn main() {}\n");
        assert!(hits.is_empty());
    }

    #[test]
    fn excerpt_is_redacted() {
        let hits = scan("AKIAIOSFODNN7EXAMPLE\n");
        assert!(hits[0].matched_excerpt.contains("***"));
        assert!(!hits[0].matched_excerpt.contains("IOSFODNN"));
    }
}
```

- [ ] **Step 2: Add `pub mod secrets;` to `lib.rs`**

- [ ] **Step 3: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core secrets::tests
```

Expected: 5 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/secrets.rs crates/core/src/lib.rs
git commit -m "feat(core): add secret scanner with gitleaks-style rule patterns"
```

---

## Task 2.3: `core::tree_sitter_compress` — code skeleton emitter

**Files:**
- Create: `crates/core/src/tree_sitter_compress.rs`
- Modify: `crates/core/Cargo.toml` (add language grammars)
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Add tree-sitter grammar dependencies**

Add to `crates/core/Cargo.toml` `[dependencies]`:

```toml
tree-sitter-rust = "0.23"
tree-sitter-python = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
```

- [ ] **Step 2: Write the test + implementation**

Create `crates/core/src/tree_sitter_compress.rs`:

```rust
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang { Rust, Python, JavaScript, TypeScript }

pub fn detect_language(path: &str) -> Option<Lang> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".rs") { Some(Lang::Rust) }
    else if lower.ends_with(".py") { Some(Lang::Python) }
    else if lower.ends_with(".ts") || lower.ends_with(".tsx") { Some(Lang::TypeScript) }
    else if lower.ends_with(".js") || lower.ends_with(".jsx") || lower.ends_with(".mjs") { Some(Lang::JavaScript) }
    else { None }
}

pub fn compress(source: &str, lang: Lang) -> String {
    let language: tree_sitter::Language = match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    };

    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() { return source.to_string(); }
    let Some(tree) = parser.parse(source, None) else { return source.to_string(); };

    let query_src = match lang {
        Lang::Rust => r#"(function_item) @item (impl_item) @item (struct_item) @item (enum_item) @item (trait_item) @item"#,
        Lang::Python => r#"(function_definition) @item (class_definition) @item"#,
        Lang::JavaScript | Lang::TypeScript => r#"(function_declaration) @item (class_declaration) @item (method_definition) @item"#,
    };
    let Ok(query) = Query::new(&language, query_src) else { return source.to_string(); };

    let mut cursor = QueryCursor::new();
    let mut out = String::new();
    let bytes = source.as_bytes();
    out.push_str(&format!("// COMPRESSED skeleton — bodies elided\n"));

    let mut matches_iter = cursor.matches(&query, tree.root_node(), bytes);
    while let Some(m) = matches_iter.next() {
        for capture in m.captures {
            let node = capture.node;
            let start = node.start_byte();
            let body_start = first_brace_or_colon(bytes, start, node.end_byte());
            let header = std::str::from_utf8(&bytes[start..body_start]).unwrap_or("").trim_end();
            out.push_str(header);
            out.push_str(" { /* … */ }\n");
        }
    }

    out
}

fn first_brace_or_colon(bytes: &[u8], start: usize, end: usize) -> usize {
    for i in start..end {
        if bytes[i] == b'{' || bytes[i] == b':' { return i; }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_extension() {
        assert_eq!(detect_language("src/main.rs"), Some(Lang::Rust));
    }

    #[test]
    fn detects_python_extension() {
        assert_eq!(detect_language("app.py"), Some(Lang::Python));
    }

    #[test]
    fn rust_skeleton_keeps_signatures_drops_bodies() {
        let src = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }\n";
        let out = compress(src, Lang::Rust);
        assert!(out.contains("fn add(a: i32, b: i32) -> i32"));
        assert!(out.contains("fn sub(a: i32, b: i32) -> i32"));
        assert!(!out.contains("a + b"));
        assert!(!out.contains("a - b"));
    }

    #[test]
    fn python_skeleton_keeps_def_lines() {
        let src = "def hello(name):\n    return f'hi, {name}'\n";
        let out = compress(src, Lang::Python);
        assert!(out.contains("def hello(name)"));
        assert!(!out.contains("return f'hi, {name}'"));
    }
}
```

> Tree-sitter API note: `set_language` takes `&LanguageRef` in 0.25+; the syntax `tree_sitter_rust::LANGUAGE.into()` produces a `Language`. If your grammar crate version doesn't expose `LANGUAGE`, fall back to the `language()` function (older API). Adjust as needed.

- [ ] **Step 3: Add `pub mod tree_sitter_compress;` to `lib.rs`**

- [ ] **Step 4: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core tree_sitter_compress::tests
```

Expected: 4 passed. If any fail because the tree-sitter API doesn't match exactly, adjust the call site to whatever the version on docs.rs requires; the test contract is what matters (skeleton keeps signatures, drops bodies).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/tree_sitter_compress.rs crates/core/Cargo.toml crates/core/src/lib.rs Cargo.lock
git commit -m "feat(core): add tree-sitter skeleton compressor for rust/python/js/ts"
```

---

## Task 2.4: `core::github` — shallow clone via gix

**Files:**
- Create: `crates/core/src/github.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the test + implementation**

Create `crates/core/src/github.rs`:

```rust
use crate::error::{CoreError, CoreResult};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGithubUrl {
    pub owner: String,
    pub repo: String,
    pub https_url: String,
}

pub fn parse_github_url(url: &str) -> CoreResult<ParsedGithubUrl> {
    let s = url.trim().trim_end_matches('/');

    let path = if let Some(rest) = s.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = s.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = s.strip_prefix("github.com/") {
        rest
    } else {
        return Err(CoreError::InvalidTarget(format!("not a github url: {url}")));
    };

    let path = path.trim_end_matches(".git");
    let mut parts = path.splitn(3, '/');
    let owner = parts.next().filter(|s| !s.is_empty()).ok_or_else(|| CoreError::InvalidTarget(format!("missing owner: {url}")))?;
    let repo = parts.next().filter(|s| !s.is_empty()).ok_or_else(|| CoreError::InvalidTarget(format!("missing repo: {url}")))?;

    Ok(ParsedGithubUrl {
        owner: owner.to_string(),
        repo: repo.to_string(),
        https_url: format!("https://github.com/{owner}/{repo}.git"),
    })
}

pub struct ClonedRepo {
    pub path: PathBuf,
    _guard: tempfile::TempDir,
}

pub fn shallow_clone(url: &str, job_id: &str) -> CoreResult<ClonedRepo> {
    let parsed = parse_github_url(url)?;
    let temp = tempfile::Builder::new()
        .prefix(&format!("projectpacker-{job_id}-"))
        .tempdir()
        .map_err(|e| CoreError::CloneFailed(format!("temp dir: {e}")))?;
    let target = temp.path().join(&parsed.repo);

    gix::prepare_clone(parsed.https_url.as_str(), &target)
        .map_err(|e| CoreError::CloneFailed(e.to_string()))?
        .with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(std::num::NonZeroU32::new(1).unwrap()))
        .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| CoreError::CloneFailed(e.to_string()))?
        .0
        .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|e| CoreError::CloneFailed(e.to_string()))?;

    Ok(ClonedRepo { path: target, _guard: temp })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_url() {
        let p = parse_github_url("https://github.com/CBaileyDev/ProjectPacker").unwrap();
        assert_eq!(p.owner, "CBaileyDev");
        assert_eq!(p.repo, "ProjectPacker");
        assert_eq!(p.https_url, "https://github.com/CBaileyDev/ProjectPacker.git");
    }

    #[test]
    fn parses_https_url_with_dot_git() {
        let p = parse_github_url("https://github.com/foo/bar.git").unwrap();
        assert_eq!(p.repo, "bar");
    }

    #[test]
    fn parses_git_at_form() {
        let p = parse_github_url("git@github.com:foo/bar.git").unwrap();
        assert_eq!(p.owner, "foo");
        assert_eq!(p.repo, "bar");
    }

    #[test]
    fn rejects_non_github_url() {
        let err = parse_github_url("https://gitlab.com/foo/bar").unwrap_err();
        assert!(matches!(err, CoreError::InvalidTarget(_)));
    }

    #[test]
    fn rejects_missing_repo() {
        let err = parse_github_url("https://github.com/owner-only").unwrap_err();
        assert!(matches!(err, CoreError::InvalidTarget(_)));
    }
}
```

- [ ] **Step 2: Add `pub mod github;` to `lib.rs`**

- [ ] **Step 3: Run tests; expect PASS** (only URL-parse tests run; no network)

```bash
cargo test -p projectpacker-core github::tests
```

Expected: 5 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/github.rs crates/core/src/lib.rs Cargo.lock
git commit -m "feat(core): add github URL parser and shallow-clone wrapper via gix"
```

> The actual `shallow_clone` function is exercised in Phase 6 integration tests against a local file:// fake remote, not in unit tests.

---

# Phase 3 — Protocol & pack assembly

## Task 3.1: Write the protocol template files

**Files:**
- Create: `docs/protocol/grok-to-cc-v1.md`

- [ ] **Step 1: Create the protocol template**

Copy the verbatim text from spec §8.3 (pack protocol block) and §8.5 (Claude Code prompt template) into `docs/protocol/grok-to-cc-v1.md`. Use the file format below — it has two named sections separated by a marker line, which the protocol module parses at compile time:

```markdown
<!-- PROTOCOL VERSION: grok-to-cc-v1 -->

===PACK_PROTOCOL_BLOCK===
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
   describing the change.>

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
===END===

===CLAUDE_CODE_PROMPT===
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
===END===
```

- [ ] **Step 2: Commit**

```bash
git add docs/protocol/grok-to-cc-v1.md
git commit -m "docs(protocol): add grok-to-cc-v1 protocol template"
```

---

## Task 3.2: `core::protocol` — block_for_pack and claude_code_prompt

**Files:**
- Create: `crates/core/src/protocol.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the test + implementation**

Create `crates/core/src/protocol.rs`:

```rust
use crate::error::{CoreError, CoreResult};

const V1: &str = include_str!("../../../docs/protocol/grok-to-cc-v1.md");

pub fn block_for_pack(goal: &str, version: &str) -> CoreResult<String> {
    let template = template_for(version)?;
    let body = extract_section(template, "PACK_PROTOCOL_BLOCK")
        .ok_or_else(|| CoreError::Internal(format!("template {version} missing PACK_PROTOCOL_BLOCK")))?;
    let mut out = String::new();
    out.push_str(&format!("<protocol version=\"{version}\">\n"));
    out.push_str(body);
    out.push_str("\n</protocol>\n");
    out.push_str("<user_task>\n");
    out.push_str(goal.trim());
    out.push_str("\n</user_task>\n");
    Ok(out)
}

pub fn claude_code_prompt(version: &str) -> CoreResult<String> {
    let template = template_for(version)?;
    let body = extract_section(template, "CLAUDE_CODE_PROMPT")
        .ok_or_else(|| CoreError::Internal(format!("template {version} missing CLAUDE_CODE_PROMPT")))?;
    Ok(body.to_string())
}

pub fn build_combined_prompt(plan_md: &str, version: &str) -> CoreResult<String> {
    let prompt = claude_code_prompt(version)?;
    let placeholder = "[The plan from Grok will be inserted here by the Bridge step.]";
    Ok(prompt.replace(placeholder, plan_md.trim()))
}

fn template_for(version: &str) -> CoreResult<&'static str> {
    match version {
        "grok-to-cc-v1" => Ok(V1),
        other => Err(CoreError::Internal(format!("unknown protocol version: {other}"))),
    }
}

fn extract_section<'a>(template: &'a str, name: &str) -> Option<&'a str> {
    let start_marker = format!("==={name}===");
    let end_marker = "===END===";
    let start = template.find(&start_marker)? + start_marker.len();
    let after = &template[start..];
    let end = after.find(end_marker)?;
    Some(after[..end].trim_matches(['\n', '\r']))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_for_pack_wraps_with_protocol_tag() {
        let s = block_for_pack("Add a feature", "grok-to-cc-v1").unwrap();
        assert!(s.starts_with("<protocol version=\"grok-to-cc-v1\">"));
        assert!(s.contains("</protocol>"));
        assert!(s.contains("<user_task>"));
        assert!(s.contains("Add a feature"));
    }

    #[test]
    fn block_for_pack_includes_strict_format_text() {
        let s = block_for_pack("hi", "grok-to-cc-v1").unwrap();
        assert!(s.contains("Plan format (STRICT)"));
        assert!(s.contains("Rationale"));
    }

    #[test]
    fn claude_code_prompt_starts_correctly() {
        let s = claude_code_prompt("grok-to-cc-v1").unwrap();
        assert!(s.contains("EXECUTOR with veto power"));
        assert!(s.contains("Challenge before executing"));
    }

    #[test]
    fn build_combined_prompt_substitutes_plan() {
        let plan = "### Summary\nA tiny plan.\n";
        let s = build_combined_prompt(plan, "grok-to-cc-v1").unwrap();
        assert!(s.contains("### Summary"));
        assert!(!s.contains("[The plan from Grok will be inserted here"));
    }

    #[test]
    fn unknown_version_errors() {
        let err = block_for_pack("hi", "grok-to-cc-v999").unwrap_err();
        assert!(matches!(err, CoreError::Internal(_)));
    }
}
```

- [ ] **Step 2: Re-enable the module in `lib.rs`**

```rust
pub mod types;
pub mod error;
pub mod ignore;
pub mod walker;
pub mod tokens;
pub mod secrets;
pub mod tree_sitter_compress;
pub mod github;
pub mod protocol;
```

- [ ] **Step 3: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core protocol::tests
```

Expected: 5 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/protocol.rs crates/core/src/lib.rs
git commit -m "feat(core): add protocol module with block_for_pack/claude_code_prompt/combined_prompt"
```

---

## Task 3.3: `core::protocol::validate_plan` — strict format validator

**Files:**
- Modify: `crates/core/src/protocol.rs`

- [ ] **Step 1: Append validator types to `protocol.rs`**

Add at the bottom of `crates/core/src/protocol.rs` (above `#[cfg(test)]`):

```rust
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanValidation {
    pub ok: bool,
    pub errors: Vec<PlanError>,
}

const REQUIRED_SECTIONS: &[&str] = &["Summary", "Risks", "Steps", "Verification", "Rollback"];
const VALID_ACTIONS: &[&str] = &["edit", "create", "delete", "rename", "run"];

pub fn validate_plan(md: &str, version: &str) -> CoreResult<PlanValidation> {
    if version != "grok-to-cc-v1" {
        return Err(CoreError::Internal(format!("unknown protocol version: {version}")));
    }
    let mut errors = Vec::new();

    let section_positions = find_sections(md);
    for (i, name) in REQUIRED_SECTIONS.iter().enumerate() {
        match section_positions.get(*name) {
            None => errors.push(PlanError {
                code: "missing_section".into(),
                message: format!("Missing section: ### {name}"),
            }),
            Some(&pos) => {
                if let Some((prev_name, &prev_pos)) = REQUIRED_SECTIONS.iter()
                    .take(i)
                    .filter_map(|n| section_positions.get(*n).map(|p| (*n, p)))
                    .last()
                {
                    if pos < prev_pos {
                        errors.push(PlanError {
                            code: "out_of_order".into(),
                            message: format!("Section ### {name} appears before ### {prev_name}"),
                        });
                    }
                }
            }
        }
    }

    let steps_text = section_text(md, &section_positions, "Steps");
    let verification_text = section_text(md, &section_positions, "Verification");

    if let Some(steps) = steps_text {
        validate_steps(steps, &mut errors);
    }

    if let Some(v) = verification_text {
        if !v.lines().any(|l| l.trim_start().starts_with("- ")) {
            errors.push(PlanError {
                code: "verification_empty".into(),
                message: "Verification section has no bullet items".into(),
            });
        }
    }

    Ok(PlanValidation { ok: errors.is_empty(), errors })
}

fn find_sections(md: &str) -> std::collections::HashMap<&'static str, usize> {
    let mut out = std::collections::HashMap::new();
    let mut byte_pos = 0usize;
    for line in md.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            let header = rest.trim_end_matches(['\r', '\n']).trim();
            for &name in REQUIRED_SECTIONS {
                if header.eq_ignore_ascii_case(name) && !out.contains_key(name) {
                    out.insert(name, byte_pos);
                }
            }
        }
        byte_pos += line.len();
    }
    out
}

fn section_text<'a>(md: &'a str, positions: &std::collections::HashMap<&'static str, usize>, name: &str) -> Option<&'a str> {
    let start = *positions.get(name)?;
    let next = REQUIRED_SECTIONS.iter()
        .filter_map(|n| positions.get(*n).copied())
        .filter(|&p| p > start)
        .min()
        .unwrap_or(md.len());
    Some(&md[start..next])
}

fn validate_steps(steps: &str, errors: &mut Vec<PlanError>) {
    let mut step_num = 0u32;
    let mut current_block = String::new();

    for line in steps.lines() {
        if line.trim_start().starts_with("#### Step ") {
            if step_num > 0 {
                check_step(step_num, &current_block, errors);
            }
            step_num += 1;
            current_block.clear();
        }
        current_block.push_str(line);
        current_block.push('\n');
    }
    if step_num > 0 {
        check_step(step_num, &current_block, errors);
    }
    if step_num == 0 {
        errors.push(PlanError {
            code: "no_steps".into(),
            message: "Steps section has no #### Step N: items".into(),
        });
    }
}

fn check_step(num: u32, block: &str, errors: &mut Vec<PlanError>) {
    let action = field(block, "Action");
    let target = field(block, "Target");
    let rationale = field(block, "Rationale");
    let has_details = block.contains("**Details:**");

    if action.is_none() {
        errors.push(PlanError { code: "missing_field".into(), message: format!("Step {num}: missing Action") });
    } else if let Some(a) = action {
        let a_norm = a.trim().to_lowercase();
        if !VALID_ACTIONS.iter().any(|v| **v == a_norm) {
            errors.push(PlanError { code: "invalid_action".into(), message: format!("Step {num}: invalid Action '{a}' (expected edit|create|delete|rename|run)") });
        }
    }

    if target.is_none() {
        errors.push(PlanError { code: "missing_field".into(), message: format!("Step {num}: missing Target") });
    }

    match rationale {
        None => errors.push(PlanError { code: "missing_field".into(), message: format!("Step {num}: missing Rationale") }),
        Some(r) if r.trim().len() < 10 => errors.push(PlanError { code: "rationale_too_short".into(), message: format!("Step {num}: Rationale must be ≥10 characters") }),
        _ => {}
    }

    if !has_details {
        errors.push(PlanError { code: "missing_field".into(), message: format!("Step {num}: missing Details") });
    }
}

fn field<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("**{name}:**");
    let idx = block.find(&needle)? + needle.len();
    let after = &block[idx..];
    let line_end = after.find('\n').unwrap_or(after.len());
    Some(after[..line_end].trim())
}
```

- [ ] **Step 2: Add validator tests at the bottom of the existing `#[cfg(test)] mod tests`**

```rust
    fn good_plan() -> &'static str {
        r#"
### Summary
A short overview.

### Risks
- None.

### Steps

#### Step 1: Add a thing
**Action:** create
**Target:** src/thing.rs
**Rationale:** This module is needed because there is currently no place for the thing logic.
**Details:**
```rust
pub fn thing() {}
```

### Verification
- `cargo test` passes.

### Rollback
- `git revert`.
"#
    }

    #[test]
    fn validates_a_correct_plan() {
        let v = validate_plan(good_plan(), "grok-to-cc-v1").unwrap();
        assert!(v.ok, "errors: {:?}", v.errors);
    }

    #[test]
    fn flags_missing_summary_section() {
        let plan = good_plan().replace("### Summary\nA short overview.", "");
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "missing_section" && e.message.contains("Summary")));
    }

    #[test]
    fn flags_missing_rationale() {
        let plan = good_plan().replace(
            "**Rationale:** This module is needed because there is currently no place for the thing logic.\n",
            "",
        );
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.message.contains("missing Rationale")));
    }

    #[test]
    fn flags_short_rationale() {
        let plan = good_plan().replace(
            "**Rationale:** This module is needed because there is currently no place for the thing logic.",
            "**Rationale:** short",
        );
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "rationale_too_short"));
    }

    #[test]
    fn flags_invalid_action() {
        let plan = good_plan().replace("**Action:** create", "**Action:** delete-everything");
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "invalid_action"));
    }

    #[test]
    fn flags_empty_verification() {
        let plan = good_plan().replace("- `cargo test` passes.", "");
        let v = validate_plan(&plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "verification_empty"));
    }

    #[test]
    fn flags_missing_steps() {
        let plan = "### Summary\nfoo\n### Risks\n- None.\n### Steps\n### Verification\n- yes\n### Rollback\n- yes\n";
        let v = validate_plan(plan, "grok-to-cc-v1").unwrap();
        assert!(!v.ok);
        assert!(v.errors.iter().any(|e| e.code == "no_steps"));
    }
```

- [ ] **Step 3: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core protocol::tests
```

Expected: 12 passed (5 from Task 3.2 + 7 new).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/protocol.rs
git commit -m "feat(core): add validate_plan with strict-format checking and per-rule errors"
```

---

## Task 3.4: `core::pack::xml` — streaming XML emission

**Files:**
- Create: `crates/core/src/pack/mod.rs`
- Create: `crates/core/src/pack/xml.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod pack;`)

- [ ] **Step 1: Create `crates/core/src/pack/mod.rs`**

```rust
pub mod xml;

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub content: String,
    pub bytes: u64,
    pub tokens: Option<u32>,
    pub hash: String,
}
```

- [ ] **Step 2: Create `crates/core/src/pack/xml.rs`**

```rust
use crate::pack::FileEntry;
use crate::types::PackStats;
use std::fmt::Write;

pub struct XmlBuilder {
    out: String,
}

impl XmlBuilder {
    pub fn new() -> Self { Self { out: String::new() } }

    pub fn open_repository(&mut self) -> &mut Self {
        self.out.push_str("<repository>\n");
        self
    }

    pub fn close_repository(&mut self) -> &mut Self {
        self.out.push_str("</repository>\n");
        self
    }

    pub fn raw_block(&mut self, body: &str) -> &mut Self {
        self.out.push_str(body);
        if !body.ends_with('\n') { self.out.push('\n'); }
        self
    }

    pub fn file_summary(&mut self, stats: &PackStats) -> &mut Self {
        let _ = writeln!(self.out, "<file_summary>");
        let _ = writeln!(self.out, "  files_total: {}", stats.files_total);
        let _ = writeln!(self.out, "  files_included: {}", stats.files_included);
        let _ = writeln!(self.out, "  files_skipped: {}", stats.files_skipped);
        let _ = writeln!(self.out, "  bytes_total: {}", stats.bytes_total);
        if let Some(t) = stats.tokens_total { let _ = writeln!(self.out, "  tokens_total: {t}"); }
        let _ = writeln!(self.out, "  secrets_found: {}", stats.secrets_found);
        let _ = writeln!(self.out, "</file_summary>");
        self
    }

    pub fn directory_structure(&mut self, paths: &[String]) -> &mut Self {
        self.out.push_str("<directory_structure>\n");
        for p in paths { self.out.push_str(p); self.out.push('\n'); }
        self.out.push_str("</directory_structure>\n");
        self
    }

    pub fn files(&mut self, files: &[FileEntry]) -> &mut Self {
        self.out.push_str("<files>\n");
        for f in files {
            let tokens_attr = match f.tokens {
                Some(t) => format!(" tokens=\"{t}\""),
                None => String::new(),
            };
            let _ = write!(
                self.out,
                "<file path=\"{}\"{tokens_attr} hash=\"{}\">\n",
                escape_attr(&f.path),
                f.hash
            );
            self.out.push_str(&escape_text(&f.content));
            if !f.content.ends_with('\n') { self.out.push('\n'); }
            self.out.push_str("</file>\n");
        }
        self.out.push_str("</files>\n");
        self
    }

    pub fn git_logs(&mut self, body: &str) -> &mut Self {
        self.out.push_str("<git_logs>\n");
        self.out.push_str(&escape_text(body));
        if !body.ends_with('\n') { self.out.push('\n'); }
        self.out.push_str("</git_logs>\n");
        self
    }

    pub fn finish(self) -> String { self.out }
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackStats;

    fn empty_stats() -> PackStats {
        PackStats { files_total: 0, files_included: 0, files_skipped: 0, bytes_total: 0, tokens_total: None, secrets_found: 0, duration_ms: 0 }
    }

    #[test]
    fn empty_repository_brackets() {
        let mut b = XmlBuilder::new();
        b.open_repository().close_repository();
        let s = b.finish();
        assert!(s.starts_with("<repository>"));
        assert!(s.ends_with("</repository>\n"));
    }

    #[test]
    fn escapes_attribute_quotes() {
        let entry = FileEntry { path: r#"a"b.txt"#.into(), content: "hi".into(), bytes: 2, tokens: None, hash: "abc".into() };
        let mut b = XmlBuilder::new();
        b.files(&[entry]);
        let s = b.finish();
        assert!(s.contains(r#"path="a&quot;b.txt""#));
    }

    #[test]
    fn escapes_text_content_lt_gt_amp() {
        let entry = FileEntry { path: "a.txt".into(), content: "<x> & </x>".into(), bytes: 11, tokens: None, hash: "abc".into() };
        let mut b = XmlBuilder::new();
        b.files(&[entry]);
        let s = b.finish();
        assert!(s.contains("&lt;x&gt; &amp; &lt;/x&gt;"));
    }

    #[test]
    fn file_summary_emits_stats_lines() {
        let mut b = XmlBuilder::new();
        let stats = PackStats { files_total: 5, files_included: 4, files_skipped: 1, bytes_total: 1024, tokens_total: Some(200), secrets_found: 0, duration_ms: 100 };
        b.file_summary(&stats);
        let s = b.finish();
        assert!(s.contains("files_total: 5"));
        assert!(s.contains("tokens_total: 200"));
    }

    #[test]
    fn _empty_helper(_unused: &PackStats) {} // referenced to silence dead_code warnings if any
    #[allow(dead_code)] fn _e() { _empty_helper(&empty_stats()); }
}
```

- [ ] **Step 3: Add `pub mod pack;` to `lib.rs`**

- [ ] **Step 4: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core pack::xml::tests
```

Expected: 4 passed (the dead-code helper doesn't run as a test).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/pack/ crates/core/src/lib.rs
git commit -m "feat(core): add streaming XML builder with proper escape handling"
```

---

## Task 3.5: `core::pack::orchestrator` — the main pack pipeline

**Files:**
- Create: `crates/core/src/pack/orchestrator.rs`
- Modify: `crates/core/src/pack/mod.rs`

- [ ] **Step 1: Add `pub mod orchestrator;` and re-export to `pack/mod.rs`**

```rust
pub mod xml;
pub mod orchestrator;

pub use orchestrator::{pack, PackEvent};

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub content: String,
    pub bytes: u64,
    pub tokens: Option<u32>,
    pub hash: String,
}
```

- [ ] **Step 2: Create `crates/core/src/pack/orchestrator.rs`**

```rust
use crate::error::{CoreError, CoreResult};
use crate::ignore::IgnoreMatcher;
use crate::pack::xml::XmlBuilder;
use crate::pack::FileEntry;
use crate::protocol;
use crate::secrets;
use crate::tokens;
use crate::tree_sitter_compress;
use crate::types::*;
use crate::walker::{self, WalkOptions};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Instant;

pub type PackEvent = ProgressEvent;

pub fn pack(
    root: &Path,
    opts: &PackOptions,
    tx: Sender<PackEvent>,
    job_id: &str,
) -> CoreResult<PackResult> {
    let start = Instant::now();
    let mut warnings: Vec<PackWarning> = Vec::new();

    let label = root.display().to_string();
    let _ = tx.send(ProgressEvent::Started { job_id: job_id.into(), target_label: label });

    let matcher = IgnoreMatcher::new(root, &opts.custom_ignore_patterns, opts.respect_gitignore);
    let outcome = walker::walk(root, &matcher, &WalkOptions { max_file_size_kb: opts.max_file_size_kb });

    let _ = tx.send(ProgressEvent::Walking { files_scanned: outcome.included.len() as u32 });
    let _ = tx.send(ProgressEvent::FileFoundBatch { paths: outcome.included.clone() });

    for (p, r) in &outcome.skipped {
        let _ = tx.send(ProgressEvent::FileSkipped { path: p.clone(), reason: r.clone() });
    }

    let _ = tx.send(ProgressEvent::BuildingXml);

    let entries: Vec<FileEntry> = outcome.included.par_iter().map(|f| {
        let abs = root.join(&f.path);
        let raw = match read_text(&abs) {
            Ok(s) => s,
            Err(_) => String::new(),
        };
        let (content, _compressed) = if opts.compress {
            if let Some(lang) = tree_sitter_compress::detect_language(&f.path) {
                (tree_sitter_compress::compress(&raw, lang), true)
            } else { (raw.clone(), false) }
        } else { (raw.clone(), false) };

        let tokens = if opts.count_tokens {
            tokens::count(&opts.tokenizer_model, &content).ok()
        } else { None };

        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        FileEntry { path: f.path.clone(), content, bytes: f.bytes, tokens, hash }
    }).collect();

    let mut secrets_found = 0u32;
    if opts.secret_scan {
        for e in &entries {
            for hit in secrets::scan(&e.content) {
                secrets_found += 1;
                let _ = tx.send(ProgressEvent::SecretHit { path: e.path.clone(), secret_kind: hit.kind, line: hit.line });
            }
        }
    }

    let mut bytes_total = 0u64;
    let mut tokens_total: u32 = 0;
    for e in &entries { bytes_total += e.bytes; if let Some(t) = e.tokens { tokens_total += t; } }

    let stats = PackStats {
        files_total: (outcome.included.len() + outcome.skipped.len()) as u32,
        files_included: entries.len() as u32,
        files_skipped: outcome.skipped.len() as u32,
        bytes_total,
        tokens_total: opts.count_tokens.then_some(tokens_total),
        secrets_found,
        duration_ms: start.elapsed().as_millis() as u32,
    };

    let dir_paths: Vec<String> = entries.iter().map(|e| e.path.clone()).collect();
    let protocol_block = protocol::block_for_pack(&opts.goal, &opts.protocol_version)?;
    let mut builder = XmlBuilder::new();
    builder
        .open_repository()
        .raw_block(&protocol_block)
        .file_summary(&stats)
        .directory_structure(&dir_paths)
        .files(&entries)
        .close_repository();
    let xml = builder.finish();

    let claude_code_prompt = protocol::claude_code_prompt(&opts.protocol_version)?;

    let _ = tx.send(ProgressEvent::Done { stats: stats.clone() });

    Ok(PackResult { xml, claude_code_prompt, stats, warnings })
}

fn read_text(path: &Path) -> CoreResult<String> {
    let bytes = std::fs::read(path).map_err(|e| CoreError::FileIo { path: path.to_path_buf(), source: e })?;
    Ok(match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            let (cow, _, _) = encoding_rs::UTF_16LE.decode(&bytes);
            cow.into_owned()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fixture() -> tempfile::TempDir {
        let d = tempdir().unwrap();
        fs::write(d.path().join("a.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
        fs::write(d.path().join("README.md"), "# title\n\nText.\n").unwrap();
        d
    }

    #[test]
    fn end_to_end_produces_xml_and_stats() {
        let d = fixture();
        let opts = PackOptions { goal: "Add a hello".into(), ..PackOptions::default() };
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = pack(d.path(), &opts, tx, "job-test").unwrap();
        assert!(result.xml.contains("<protocol version=\"grok-to-cc-v1\">"));
        assert!(result.xml.contains("<files>"));
        assert!(result.xml.contains("README.md"));
        assert!(result.xml.contains("a.rs"));
        assert_eq!(result.stats.files_included, 2);
        assert!(result.claude_code_prompt.contains("EXECUTOR with veto power"));
    }

    #[test]
    fn emits_progress_events_in_expected_order() {
        let d = fixture();
        let opts = PackOptions { goal: "x".into(), count_tokens: false, secret_scan: false, ..PackOptions::default() };
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = pack(d.path(), &opts, tx, "job-test").unwrap();
        let mut events: Vec<&'static str> = Vec::new();
        for ev in rx.try_iter() {
            events.push(match ev {
                ProgressEvent::Started { .. } => "started",
                ProgressEvent::Walking { .. } => "walking",
                ProgressEvent::FileFoundBatch { .. } => "batch",
                ProgressEvent::FileSkipped { .. } => "skipped",
                ProgressEvent::BuildingXml => "building",
                ProgressEvent::Done { .. } => "done",
                _ => "other",
            });
        }
        assert_eq!(events.first(), Some(&"started"));
        assert_eq!(events.last(), Some(&"done"));
        assert!(events.contains(&"building"));
    }
}
```

- [ ] **Step 3: Run tests; expect PASS**

```bash
cargo test -p projectpacker-core pack::orchestrator::tests
```

Expected: 2 passed.

- [ ] **Step 4: Run the full core test suite to make sure nothing regressed**

```bash
cargo test -p projectpacker-core
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/pack/
git commit -m "feat(core): add pack orchestrator wiring walker→processors→xml→protocol"
```

---

# Phase 4 — Tauri commands & wiring

## Task 4.1: App settings module

**Files:**
- Create: `crates/app/src/settings.rs`

- [ ] **Step 1: Write the test + implementation**

Create `crates/app/src/settings.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    pub default_protocol_version: String,
    pub default_tokenizer_model: String,
    pub recents: Vec<Recent>,
    pub goal_templates: Vec<GoalTemplate>,
    pub presets: Vec<Preset>,
}

impl Settings {
    pub fn defaults() -> Self {
        Self {
            theme: Theme::Dark,
            default_protocol_version: "grok-to-cc-v1".into(),
            default_tokenizer_model: "gpt-4o-mini".into(),
            recents: Vec::new(),
            goal_templates: Vec::new(),
            presets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub label: String,
    pub target: String,
    pub last_used_iso: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GoalTemplate {
    pub name: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    pub options_json: String,
}

pub fn load_or_default(path: &PathBuf) -> Settings {
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(s) = serde_json::from_str::<Settings>(&text) {
            return s;
        }
        let bad = path.with_extension(format!("json.bad-{}", chrono_isoish_now()));
        let _ = std::fs::rename(path, bad);
    }
    Settings::defaults()
}

pub fn save(path: &PathBuf, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings).unwrap();
    std::fs::write(path, json)
}

fn chrono_isoish_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_round_trip_through_json() {
        let s = Settings::defaults();
        let j = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(back.theme, Theme::Dark);
        assert_eq!(back.default_tokenizer_model, "gpt-4o-mini");
    }

    #[test]
    fn load_returns_default_when_missing() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        let s = load_or_default(&path);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
    }

    #[test]
    fn save_then_load_returns_same_data() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        let mut s = Settings::defaults();
        s.recents.push(Recent { label: "x".into(), target: "/tmp/x".into(), last_used_iso: "now".into() });
        save(&path, &s).unwrap();
        let back = load_or_default(&path);
        assert_eq!(back.recents.len(), 1);
        assert_eq!(back.recents[0].label, "x");
    }

    #[test]
    fn load_recovers_from_corrupt_file() {
        let d = tempdir().unwrap();
        let path = d.path().join("settings.json");
        std::fs::write(&path, "this is not json").unwrap();
        let s = load_or_default(&path);
        assert_eq!(s.default_protocol_version, "grok-to-cc-v1");
        assert!(!path.exists() || std::fs::read_to_string(&path).unwrap().contains("\"theme\""));
    }
}
```

- [ ] **Step 2: Wire `mod settings;` into `crates/app/src/lib.rs`**

```rust
//! ProjectPacker Tauri shell.

pub mod settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run tests; expect PASS**

```bash
cargo test -p projectpacker-app settings::tests
```

Expected: 4 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/app/src/settings.rs crates/app/src/lib.rs
git commit -m "feat(app): add settings module with corrupt-file recovery"
```

---

## Task 4.2: Pack-job state + commands

**Files:**
- Create: `crates/app/src/jobs.rs`
- Create: `crates/app/src/commands.rs`
- Modify: `crates/app/src/lib.rs`

- [ ] **Step 1: Create `crates/app/src/jobs.rs`**

```rust
use dashmap::DashMap;
use parking_lot::Mutex;
use projectpacker_core::types::PackResult;
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct JobRegistry {
    handles: DashMap<String, Arc<Mutex<Option<JoinHandle<()>>>>>,
    results: DashMap<String, PackResult>,
}

impl JobRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&self, job_id: &str, handle: JoinHandle<()>) {
        self.handles.insert(job_id.to_string(), Arc::new(Mutex::new(Some(handle))));
    }

    pub fn cancel(&self, job_id: &str) -> bool {
        if let Some(entry) = self.handles.get(job_id) {
            if let Some(h) = entry.lock().take() { h.abort(); return true; }
        }
        false
    }

    pub fn store_result(&self, job_id: &str, result: PackResult) {
        self.results.insert(job_id.to_string(), result);
    }

    pub fn take_result(&self, job_id: &str) -> Option<PackResult> {
        self.results.remove(job_id).map(|(_, v)| v)
    }
}
```

- [ ] **Step 2: Create `crates/app/src/commands.rs`**

```rust
use crate::jobs::JobRegistry;
use crate::settings::{load_or_default, save, Settings};
use projectpacker_core::error::CoreError;
use projectpacker_core::pack;
use projectpacker_core::protocol::{self, PlanValidation};
use projectpacker_core::types::{PackOptions, PackResult, ProgressEvent};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl From<CoreError> for AppError {
    fn from(e: CoreError) -> Self {
        let code = match &e {
            CoreError::InvalidTarget(_) => "invalid_target",
            CoreError::PathNotFound(_) => "path_not_found",
            CoreError::CloneFailed(_) => "clone_failed",
            CoreError::TokenizerUnavailable(_) => "tokenizer_unavailable",
            CoreError::PlanInvalid { .. } => "plan_invalid",
            CoreError::Cancelled => "cancelled",
            _ => "internal",
        }.to_string();
        AppError { code, message: e.to_string(), details: None }
    }
}

pub type CmdResult<T> = Result<T, AppError>;

#[tauri::command]
#[specta::specta]
pub async fn pack_start(
    app: AppHandle,
    registry: State<'_, Arc<JobRegistry>>,
    opts: PackOptions,
) -> CmdResult<String> {
    let job_id = Uuid::now_v7().to_string();
    let registry_arc = registry.inner().clone();
    let registry_for_task = registry_arc.clone();
    let app_for_emit = app.clone();
    let id = job_id.clone();
    let id_for_task = id.clone();

    // Reject GitHub URLs in v0.1.0 (URL parsing/clone exist in core::github but
    // are not wired through the orchestrator yet — see follow-up plan).
    if matches!(opts.target, projectpacker_core::types::PackTarget::GitHub(_)) {
        return Err(AppError {
            code: "not_implemented".into(),
            message: "GitHub URL packing is deferred to v0.2.0. Use a local folder.".into(),
            details: None,
        });
    }

    let (tx, rx) = std::sync::mpsc::channel::<ProgressEvent>();

    std::thread::spawn(move || {
        for ev in rx {
            let topic = format!("pack:{id}:progress");
            let _ = app_for_emit.emit(&topic, ev);
        }
    });

    let handle = tokio::task::spawn_blocking(move || {
        let root = match &opts.target {
            projectpacker_core::types::PackTarget::Folder(p) => p.clone(),
            projectpacker_core::types::PackTarget::GitHub(_) => return, // unreachable — guarded above
        };
        if let Ok(result) = pack::pack(&root, &opts, tx, &id_for_task) {
            registry_for_task.store_result(&id_for_task, result);
        }
        // Failure path: orchestrator did not emit Done; UI distinguishes by
        // absence-of-Done plus a timeout. Better error plumbing is a follow-up.
    });

    registry_arc.register(&job_id, handle);
    Ok(job_id)
}

#[tauri::command]
#[specta::specta]
pub async fn pack_cancel(registry: State<'_, Arc<JobRegistry>>, job_id: String) -> CmdResult<()> {
    registry.inner().cancel(&job_id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn pack_get_result(registry: State<'_, Arc<JobRegistry>>, job_id: String) -> CmdResult<PackResult> {
    registry.inner()
        .take_result(&job_id)
        .ok_or(AppError { code: "result_not_ready".into(), message: format!("no result for job {job_id}"), details: None })
}

#[tauri::command]
#[specta::specta]
pub async fn validate_plan(plan_md: String, protocol_version: String) -> CmdResult<PlanValidation> {
    protocol::validate_plan(&plan_md, &protocol_version).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn build_combined_prompt(plan_md: String, protocol_version: String) -> CmdResult<String> {
    protocol::build_combined_prompt(&plan_md, &protocol_version).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(app: AppHandle) -> CmdResult<Settings> {
    Ok(load_or_default(&settings_path(&app)))
}

#[tauri::command]
#[specta::specta]
pub async fn save_settings(app: AppHandle, settings: Settings) -> CmdResult<Settings> {
    save(&settings_path(&app), &settings).map_err(|e| AppError {
        code: "settings_save_failed".into(),
        message: e.to_string(),
        details: None,
    })?;
    Ok(settings)
}

#[tauri::command]
#[specta::specta]
pub async fn save_to_file(path: PathBuf, contents: String) -> CmdResult<()> {
    std::fs::write(&path, contents).map_err(|e| AppError {
        code: "save_failed".into(),
        message: e.to_string(),
        details: None,
    })
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("settings.json")
}
```

- [ ] **Step 3: Wire commands and registry into `crates/app/src/lib.rs`**

```rust
//! ProjectPacker Tauri shell.

pub mod commands;
pub mod jobs;
pub mod settings;

use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let registry = Arc::new(jobs::JobRegistry::new());

    tauri::Builder::default()
        .manage(registry)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::pack_start,
            commands::pack_cancel,
            commands::pack_get_result,
            commands::validate_plan,
            commands::build_combined_prompt,
            commands::get_settings,
            commands::save_settings,
            commands::save_to_file,
        ])
        .setup(|app| {
            tracing::info!("ProjectPacker started, version {}", env!("CARGO_PKG_VERSION"));
            let _ = app;
            std::panic::set_hook(Box::new(|info| {
                tracing::error!("PANIC in app process: {info}");
            }));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Verify the app compiles**

```bash
cargo check -p projectpacker-app
```

If `tauri::Emitter` import errors, verify Tauri 2 emit API matches; on Tauri 2, the trait is `tauri::Emitter` and `app.emit(topic, payload)` is correct.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/jobs.rs crates/app/src/commands.rs crates/app/src/lib.rs
git commit -m "feat(app): add pack/bridge/settings tauri commands and job registry"
```

> GitHub-clone path for `pack_start` is intentionally a no-op stub here; it's wired in Task 4.4 once the cancellation flow is solid.

---

## Task 4.3: Specta — emit TypeScript bindings

**Files:**
- Create: `crates/app/src/bin/emit-bindings.rs`
- Modify: `crates/app/src/commands.rs` (add specta builder helper)
- Modify: `frontend/.gitignore-aware path` (already gitignored)

- [ ] **Step 1: Add a binding-emit binary**

Create `crates/app/src/bin/emit-bindings.rs`:

```rust
use specta_typescript::Typescript;
use tauri_specta::{collect_commands, Builder};

fn main() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            projectpacker_app_lib::commands::pack_start,
            projectpacker_app_lib::commands::pack_cancel,
            projectpacker_app_lib::commands::pack_get_result,
            projectpacker_app_lib::commands::validate_plan,
            projectpacker_app_lib::commands::build_combined_prompt,
            projectpacker_app_lib::commands::get_settings,
            projectpacker_app_lib::commands::save_settings,
            projectpacker_app_lib::commands::save_to_file,
        ]);

    builder
        .export(
            Typescript::default().header("// Auto-generated by ProjectPacker — do not edit.\n"),
            "../../frontend/src/bindings/index.ts",
        )
        .expect("Failed to export typescript bindings");
}
```

- [ ] **Step 2: Add helper export in `commands.rs`**

The commands and types must be `pub`. They are.

- [ ] **Step 3: Run the binding emitter**

```bash
mkdir -p frontend/src/bindings
cargo run -p projectpacker-app --bin emit-bindings
```

Expected: `frontend/src/bindings/index.ts` is created with TypeScript types for `PackOptions`, `PackResult`, `PackStats`, `PlanValidation`, etc., plus typed wrappers for each command.

Open the file and confirm it contains exports for the types and functions like `packStart`, `validatePlan`, `buildCombinedPrompt`. If it does not, check the specta + tauri-specta versions and the trait derives in `core`.

- [ ] **Step 4: Add an npm script to regenerate bindings**

Edit `frontend/package.json` `scripts`:

```json
"bindings": "cd .. && cargo run -p projectpacker-app --bin emit-bindings",
```

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/bin/emit-bindings.rs frontend/package.json
git commit -m "feat(app): add specta TS bindings emitter binary + pnpm script"
```

---

# Phase 5 — Minimal placeholder UI

> The goal of Phase 5 is **proving the end-to-end pipeline works**, not building polished UI. Components here are deliberately ugly. Polished components arrive from Claude Design in Phase 7.

## Task 5.1: Frontend lib (api, events, store, router)

**Files:**
- Create: `frontend/src/lib/api.ts`
- Create: `frontend/src/lib/events.ts`
- Create: `frontend/src/lib/store.ts`
- Modify: `frontend/src/App.tsx`

- [ ] **Step 1: Create `frontend/src/lib/api.ts`**

```ts
import * as bindings from "../bindings";

export const api = bindings;
export type { PackOptions, PackResult, PackStats, PlanValidation, ProgressEvent, Settings } from "../bindings";
```

(Generated bindings already contain typed function wrappers; this is a re-export.)

- [ ] **Step 2: Create `frontend/src/lib/events.ts`**

```ts
import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";
import type { ProgressEvent } from "./api";

export function subscribePackProgress(
  jobId: string,
  onEvent: (e: ProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProgressEvent>(`pack:${jobId}:progress`, (e: Event<ProgressEvent>) => onEvent(e.payload));
}
```

- [ ] **Step 3: Create `frontend/src/lib/store.ts`**

```ts
import { create } from "zustand";
import type { PackOptions, PackResult, ProgressEvent } from "./api";

type PackingStatus = "idle" | "running" | "done" | "error";

interface AppState {
  jobId: string | null;
  status: PackingStatus;
  events: ProgressEvent[];
  result: PackResult | null;
  options: PackOptions;
  setJob: (id: string) => void;
  pushEvent: (e: ProgressEvent) => void;
  setResult: (r: PackResult) => void;
  reset: () => void;
  setOptions: (o: PackOptions) => void;
}

const defaultOptions: PackOptions = {
  target: { kind: "folder", value: "" } as any,
  goal: "",
  includeGitHistory: false,
  countTokens: true,
  tokenizerModel: "gpt-4o-mini",
  secretScan: true,
  compress: false,
  removeComments: false,
  maxFileSizeKb: 1024,
  respectGitignore: true,
  customIgnorePatterns: [],
  protocolVersion: "grok-to-cc-v1",
};

export const useApp = create<AppState>((set) => ({
  jobId: null,
  status: "idle",
  events: [],
  result: null,
  options: defaultOptions,
  setJob: (id) => set({ jobId: id, status: "running", events: [], result: null }),
  pushEvent: (e) => set((s) => ({
    events: [...s.events, e],
    status: e.kind === "done" ? "done" : e.kind === "error" ? "error" : s.status,
  })),
  setResult: (r) => set({ result: r }),
  reset: () => set({ jobId: null, status: "idle", events: [], result: null }),
  setOptions: (o) => set({ options: o }),
}));
```

- [ ] **Step 4: Replace `frontend/src/App.tsx` with a minimal router**

```tsx
import { HashRouter, Link, Route, Routes } from "react-router-dom";
import Home from "./routes/Home";
import Pack from "./routes/Pack";
import Result from "./routes/Result";
import Bridge from "./routes/Bridge";

export default function App() {
  return (
    <HashRouter>
      <div className="flex h-full flex-col">
        <nav className="border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-sm">
          <Link className="mr-4 underline" to="/">Home</Link>
          <Link className="mr-4 underline" to="/pack">Pack</Link>
          <Link className="mr-4 underline" to="/bridge">Bridge</Link>
        </nav>
        <main className="flex-1 overflow-auto p-4">
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/pack" element={<Pack />} />
            <Route path="/result" element={<Result />} />
            <Route path="/bridge" element={<Bridge />} />
          </Routes>
        </main>
      </div>
    </HashRouter>
  );
}
```

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib frontend/src/App.tsx
git commit -m "feat(frontend): add api/events/store helpers and minimal router"
```

---

## Task 5.2: Placeholder routes

**Files:**
- Create: `frontend/src/routes/Home.tsx`
- Create: `frontend/src/routes/Pack.tsx`
- Create: `frontend/src/routes/Result.tsx`
- Create: `frontend/src/routes/Bridge.tsx`

- [ ] **Step 1: Create `Home.tsx`**

```tsx
import { Link } from "react-router-dom";

export default function Home() {
  return (
    <div className="space-y-3">
      <h1 className="text-2xl">Home</h1>
      <Link to="/pack" className="inline-block rounded bg-zinc-800 px-3 py-1 hover:bg-zinc-700">New Pack</Link>
      <Link to="/bridge" className="ml-2 inline-block rounded bg-zinc-800 px-3 py-1 hover:bg-zinc-700">Bridge</Link>
    </div>
  );
}
```

- [ ] **Step 2: Create `Pack.tsx`** (functional, ugly)

```tsx
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { commands } from "../bindings";
import { useApp } from "../lib/store";
import { subscribePackProgress } from "../lib/events";

export default function Pack() {
  const nav = useNavigate();
  const { options, setOptions, status, events, setJob, pushEvent, setResult } = useApp();
  const [busy, setBusy] = useState(false);

  async function pickFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string") {
      setOptions({ ...options, target: { kind: "folder", value: path } as any });
    }
  }

  async function runPack() {
    setBusy(true);
    const start = await commands.packStart(options);
    if (start.status === "ok") {
      const jobId = start.data;
      setJob(jobId);
      const unlisten = await subscribePackProgress(jobId, (e) => {
        pushEvent(e);
        if (e.kind === "done") {
          (async () => {
            const r = await commands.packGetResult(jobId);
            if (r.status === "ok") setResult(r.data);
            unlisten();
            nav("/result");
          })();
        }
      });
    }
    setBusy(false);
  }

  const targetVal = (options.target as any).value ?? "";

  return (
    <div className="space-y-4">
      <h1 className="text-2xl">Pack</h1>
      <div className="space-y-2">
        <label className="block text-sm">Target folder</label>
        <div className="flex gap-2">
          <input
            className="flex-1 rounded bg-zinc-800 px-2 py-1"
            value={targetVal}
            onChange={(e) => setOptions({ ...options, target: { kind: "folder", value: e.target.value } as any })}
          />
          <button className="rounded bg-zinc-700 px-3 py-1" onClick={pickFolder}>Browse…</button>
        </div>
      </div>
      <div>
        <label className="block text-sm">Goal</label>
        <textarea
          className="h-24 w-full rounded bg-zinc-800 p-2"
          value={options.goal}
          onChange={(e) => setOptions({ ...options, goal: e.target.value })}
        />
      </div>
      <button
        className="rounded bg-emerald-700 px-4 py-2 hover:bg-emerald-600 disabled:opacity-50"
        onClick={runPack}
        disabled={busy || !targetVal}
      >
        {busy ? "Packing…" : "Pack"}
      </button>
      {status === "running" && (
        <pre className="max-h-64 overflow-auto rounded bg-zinc-900 p-2 text-xs">
          {events.map((e, i) => <div key={i}>{JSON.stringify(e)}</div>)}
        </pre>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Create `Result.tsx`**

```tsx
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useApp } from "../lib/store";

export default function Result() {
  const { result } = useApp();
  if (!result) return <div>No result. Go pack something.</div>;
  return (
    <div className="space-y-4">
      <h1 className="text-2xl">Result</h1>
      <div className="text-sm text-zinc-300">
        {result.stats.filesIncluded} files · {result.stats.bytesTotal} bytes
        {result.stats.tokensTotal != null && <> · {result.stats.tokensTotal} tokens</>}
      </div>
      <div className="flex gap-2">
        <button className="rounded bg-zinc-700 px-3 py-1" onClick={() => writeText(result.xml)}>Copy Pack XML</button>
        <button className="rounded bg-zinc-700 px-3 py-1" onClick={() => writeText(result.claudeCodePrompt)}>Copy Claude Code Prompt</button>
      </div>
      <details>
        <summary className="cursor-pointer">Pack XML preview</summary>
        <pre className="max-h-96 overflow-auto rounded bg-zinc-900 p-2 text-xs">{result.xml}</pre>
      </details>
    </div>
  );
}
```

- [ ] **Step 4: Create `Bridge.tsx`**

```tsx
import { useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { commands } from "../bindings";

export default function Bridge() {
  const [plan, setPlan] = useState("");
  const [errors, setErrors] = useState<{ code: string; message: string }[]>([]);
  const [combined, setCombined] = useState<string | null>(null);

  async function check() {
    setErrors([]); setCombined(null);
    const v = await commands.validatePlan(plan, "grok-to-cc-v1");
    if (v.status !== "ok") return;
    if (!v.data.ok) { setErrors(v.data.errors as any); return; }
    const w = await commands.buildCombinedPrompt(plan, "grok-to-cc-v1");
    if (w.status === "ok") setCombined(w.data);
  }

  return (
    <div className="space-y-4">
      <h1 className="text-2xl">Bridge</h1>
      <textarea
        className="h-64 w-full rounded bg-zinc-800 p-2 font-mono text-sm"
        value={plan}
        onChange={(e) => setPlan(e.target.value)}
        placeholder="Paste Grok's plan here…"
      />
      <button className="rounded bg-emerald-700 px-4 py-2 hover:bg-emerald-600" onClick={check}>
        Validate & Build Prompt
      </button>
      {errors.length > 0 && (
        <div className="rounded border border-red-600 bg-red-950 p-2 text-sm">
          {errors.map((e, i) => <div key={i}>• {e.message}</div>)}
        </div>
      )}
      {combined && (
        <div className="space-y-2">
          <button className="rounded bg-zinc-700 px-3 py-1" onClick={() => writeText(combined)}>Copy Combined Prompt</button>
          <pre className="max-h-64 overflow-auto rounded bg-zinc-900 p-2 text-xs">{combined}</pre>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Run the dev app and smoke-test the round trip**

```bash
pnpm tauri dev
```

Expected: window opens. Navigate to Pack, browse to a small local folder, click Pack. Watch progress events render. Result page renders. Click "Copy Pack XML" — clipboard receives the XML. Then Bridge: paste a known-good plan (use the example from spec §8.4). Click Validate. Combined prompt appears. Copy works.

If anything is broken, fix it before committing. This is the first end-to-end smoke test of the entire architecture.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/routes
git commit -m "feat(frontend): add minimal placeholder Home/Pack/Result/Bridge routes"
```

---

# Phase 6 — Test suite hardening

## Task 6.1: Build the `tiny` fixture and integration test

**Files:**
- Create: `tests/fixtures/tiny/...` (small file tree)
- Create: `tests/fixtures/tiny/.gitignore`
- Create: `tests/fixtures/tiny/README.md`
- Create: `tests/fixtures/tiny/src/main.rs`
- Create: `tests/fixtures/tiny/src/util.rs`
- Create: `tests/fixtures/tiny/.codeparserignore`
- Create: `crates/core/tests/pack_integration.rs`

- [ ] **Step 1: Build the fixture**

```bash
mkdir -p tests/fixtures/tiny/src tests/fixtures/tiny/docs tests/fixtures/tiny/build
echo "# Tiny Fixture" > tests/fixtures/tiny/README.md
echo "fn main() { println!(\"hi\"); }" > tests/fixtures/tiny/src/main.rs
echo "pub fn add(a: i32, b: i32) -> i32 { a + b }" > tests/fixtures/tiny/src/util.rs
echo "Welcome to docs." > tests/fixtures/tiny/docs/intro.md
echo "BUILD ARTIFACT" > tests/fixtures/tiny/build/output.txt
echo "build/" > tests/fixtures/tiny/.gitignore
echo "docs/private/" > tests/fixtures/tiny/.codeparserignore
echo "AKIA0000000000000000  # fake aws key" > tests/fixtures/tiny/src/danger.txt
```

- [ ] **Step 2: Allowlist the fake secret**

Create `tests/fixtures/tiny/.gitleaksignore`:

```
src/danger.txt
```

(GitHub Secret Scanning won't ignore this; we deliberately keep it in the repo since it's a known-fake key.)

- [ ] **Step 3: Create the integration test**

Create `crates/core/tests/pack_integration.rs`:

```rust
use projectpacker_core::pack;
use projectpacker_core::types::*;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().unwrap().parent().unwrap().join("tests/fixtures").join(name)
}

#[test]
fn tiny_fixture_packs_with_expected_files() {
    let root = fixture_path("tiny");
    let opts = PackOptions { goal: "test".into(), ..PackOptions::default() };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(&root, &opts, tx, "test-job").unwrap();

    assert!(result.xml.contains("README.md"));
    assert!(result.xml.contains("src/main.rs"));
    assert!(result.xml.contains("src/util.rs"));
    assert!(result.xml.contains("docs/intro.md"));
    assert!(!result.xml.contains("build/output.txt"), "build/ should be gitignored");
}

#[test]
fn tiny_fixture_detects_secret() {
    let root = fixture_path("tiny");
    let opts = PackOptions { goal: "test".into(), ..PackOptions::default() };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(&root, &opts, tx, "test-job").unwrap();
    assert!(result.stats.secrets_found >= 1, "expected at least one secret hit");
}

#[test]
fn tiny_fixture_includes_protocol_block() {
    let root = fixture_path("tiny");
    let opts = PackOptions { goal: "Add docs".into(), ..PackOptions::default() };
    let (tx, _rx) = std::sync::mpsc::channel();
    let result = pack::pack(&root, &opts, tx, "test-job").unwrap();
    assert!(result.xml.contains("<protocol version=\"grok-to-cc-v1\">"));
    assert!(result.xml.contains("<user_task>"));
    assert!(result.xml.contains("Add docs"));
}
```

- [ ] **Step 4: Run integration tests; expect PASS**

```bash
cargo test -p projectpacker-core --test pack_integration
```

Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/tiny crates/core/tests/pack_integration.rs
git commit -m "test(core): add tiny fixture + pack integration tests"
```

---

## Task 6.2: Insta snapshot tests for protocol golden files

**Files:**
- Create: `crates/core/tests/protocol_golden.rs`

- [ ] **Step 1: Add `insta` to dev-dependencies** (already in Cargo.toml from Task 0.2 — verify)

- [ ] **Step 2: Create the golden test**

```rust
use projectpacker_core::protocol;

#[test]
fn protocol_block_for_pack_v1_is_frozen() {
    let s = protocol::block_for_pack("Add a hello endpoint", "grok-to-cc-v1").unwrap();
    insta::assert_snapshot!("v1_pack_block", s);
}

#[test]
fn protocol_claude_code_prompt_v1_is_frozen() {
    let s = protocol::claude_code_prompt("grok-to-cc-v1").unwrap();
    insta::assert_snapshot!("v1_cc_prompt", s);
}

#[test]
fn protocol_combined_prompt_with_known_plan_is_frozen() {
    let plan = r#"### Summary
A tiny plan.

### Risks
- None.

### Steps

#### Step 1: Do the thing
**Action:** create
**Target:** src/thing.rs
**Rationale:** This is needed for the feature to exist.
**Details:**
```rust
pub fn thing() {}
```

### Verification
- `cargo test` passes.

### Rollback
- `git revert`.
"#;
    let s = protocol::build_combined_prompt(plan, "grok-to-cc-v1").unwrap();
    insta::assert_snapshot!("v1_combined_prompt", s);
}
```

- [ ] **Step 3: Run, then accept snapshots**

```bash
cargo test -p projectpacker-core --test protocol_golden
cargo install cargo-insta --locked
cargo insta accept
```

Expected: three `.snap` files appear under `crates/core/tests/snapshots/`.

- [ ] **Step 4: Run again to confirm green**

```bash
cargo test -p projectpacker-core --test protocol_golden
```

Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/protocol_golden.rs crates/core/tests/snapshots/
git commit -m "test(core): add frozen snapshot tests for protocol v1 outputs"
```

> **Frozen forever:** any change to these snapshots is a protocol breaking change. New behavior goes in a new protocol version (e.g., `grok-to-cc-v2`).

---

## Task 6.3: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the CI workflow**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with: { version: 9 }

      - name: Setup Node
        uses: actions/setup-node@v4
        with: { node-version: 20, cache: pnpm }

      - name: Install frontend deps
        run: pnpm install --frozen-lockfile

      - name: cargo fmt
        run: cargo fmt --all -- --check

      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: cargo test
        run: cargo test --workspace --no-fail-fast

      - name: emit bindings
        run: cargo run -p projectpacker-app --bin emit-bindings

      - name: typecheck frontend
        run: pnpm --filter projectpacker-frontend typecheck

      - name: vitest
        run: pnpm --filter projectpacker-frontend test
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add Windows-only test workflow"
```

> Push triggers CI on the next push. Confirm the green checkmark on GitHub before declaring this task done.

---

# Phase 7 — Claude Design handoff

## Task 7.1: Build the Claude Design prompt

**Files:**
- Create: `docs/handoff/claude-design-prompt.md`

- [ ] **Step 1: Generate fresh bindings (must reflect the latest types)**

```bash
cargo run -p projectpacker-app --bin emit-bindings
```

- [ ] **Step 2: Create `docs/handoff/claude-design-prompt.md`**

Copy the prompt from spec §14, then in the two `[Paste …]` markers, paste:
- (a) the contents of `frontend/src/bindings/index.ts`
- (b) the Component inventory table from spec §13

This file is a *ready-to-paste* prompt for Claude Design — open it, copy its contents, paste into a new Claude Design conversation.

- [ ] **Step 3: Commit**

```bash
git add docs/handoff/claude-design-prompt.md
git commit -m "docs(handoff): add ready-to-paste Claude Design prompt with bindings"
```

- [ ] **Step 4: Tell the user**

Output to the user:

> "ProjectPacker scaffold is functional end-to-end. The Claude Design prompt is ready at `docs/handoff/claude-design-prompt.md`. Open it, copy its contents, paste into a new Claude Design conversation, and start the Pack route. As components arrive from Claude Design, save them under `frontend/src/components/` or replace `frontend/src/routes/Pack.tsx` and re-run `pnpm tauri dev` to see them live."

---

# Phase 8 — Packaging & v0.1.0 release

## Task 8.1: Build script and release workflow

**Files:**
- Create: `scripts/build-release.ps1`
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create `scripts/build-release.ps1`**

```powershell
param([Parameter(Mandatory=$true)][string]$Version)

$ErrorActionPreference = 'Stop'

Write-Host "Building ProjectPacker v$Version" -ForegroundColor Cyan

pnpm install --frozen-lockfile
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p projectpacker-app --bin emit-bindings
pnpm --filter projectpacker-frontend typecheck

pnpm tauri build --bundles msi,nsis

$out = "dist"
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out | Out-Null }

$msi = Get-ChildItem "crates/app/target/release/bundle/msi/*.msi" | Select-Object -First 1
$exe = Get-ChildItem "crates/app/target/release/bundle/nsis/*.exe" | Select-Object -First 1

Copy-Item $msi.FullName "$out/ProjectPacker_${Version}_x64-setup.msi"
Copy-Item $exe.FullName "$out/ProjectPacker_${Version}_x64-portable.exe"

Get-FileHash "$out/ProjectPacker_${Version}_x64-setup.msi" -Algorithm SHA256 | Format-List
Get-FileHash "$out/ProjectPacker_${Version}_x64-portable.exe" -Algorithm SHA256 | Format-List

Write-Host "Done. Artifacts in $out" -ForegroundColor Green
```

- [ ] **Step 2: Create `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags: ["v*.*.*"]

permissions:
  contents: write

jobs:
  release:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
      - uses: pnpm/action-setup@v4
        with: { version: 9 }
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: pnpm }

      - run: pnpm install --frozen-lockfile
      - run: cargo test --workspace
      - run: cargo run -p projectpacker-app --bin emit-bindings
      - run: pnpm tauri build --bundles msi,nsis

      - name: Collect artifacts
        run: |
          mkdir dist
          $version = "${{ github.ref_name }}".TrimStart('v')
          Copy-Item crates/app/target/release/bundle/msi/*.msi "dist/ProjectPacker_${version}_x64-setup.msi"
          Copy-Item crates/app/target/release/bundle/nsis/*.exe "dist/ProjectPacker_${version}_x64-portable.exe"
          Get-FileHash dist/*.msi -Algorithm SHA256 | Format-List | Out-File dist/SHA256SUMS.txt -Append
          Get-FileHash dist/*.exe -Algorithm SHA256 | Format-List | Out-File dist/SHA256SUMS.txt -Append
        shell: pwsh

      - uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/*.msi
            dist/*.exe
            dist/SHA256SUMS.txt
          draft: true
          generate_release_notes: true
```

- [ ] **Step 3: Commit**

```bash
git add scripts/build-release.ps1 .github/workflows/release.yml
git commit -m "build: add local release script and tag-triggered release workflow"
```

---

## Task 8.2: Cut v0.1.0 (manual)

- [ ] **Step 1: Update CHANGELOG.md**

Move the `## [Unreleased]` items to a new `## [0.1.0] - 2026-04-30` section. Add a new empty `## [Unreleased]` at the top.

- [ ] **Step 2: Run a dry-run local build to confirm release compiles**

```bash
./scripts/build-release.ps1 0.1.0
```

Expected: `dist/ProjectPacker_0.1.0_x64-setup.msi` and `dist/ProjectPacker_0.1.0_x64-portable.exe` exist; SHA256 hashes printed.

- [ ] **Step 3: Commit changelog**

```bash
git add CHANGELOG.md
git commit -m "chore: cut v0.1.0"
```

- [ ] **Step 4: Tag and prepare push**

```bash
git tag v0.1.0
```

**The actual `git push origin main --tags`** is a manual step the user runs. Print this command for the user:

> `git push origin main --tags`

Confirm with the user that the release workflow on GitHub completed successfully (a draft release with the .msi, .exe, and SHA256SUMS appears under Releases).

- [ ] **Step 5: User publishes the draft release** (manual, in the GitHub UI).

---

## Done

Stop here. v0.1.0 is shipped. Next steps (handled in their own plans) are:

- Receive Claude Design components and integrate them.
- Iterate on protocol grammar based on real Grok responses.
- Add cross-platform builds, CLI binary, auto-update — see spec §12.

---

## Self-review notes

This plan covers all goals from spec §2 and explicitly defers all non-goals from §3. Coverage map:

- Pack folder/GitHub URL with ignore handling → Tasks 1.3, 1.4, 2.4, 3.5
- Embed strict protocol with rationale → Tasks 3.1, 3.2, 3.3
- Generate Claude Code prompt → Task 3.2
- Validate planner output → Task 3.3
- Animation-rich UI room → Phase 7 handoff (foundation in 5.x is intentionally minimal)
- V1 feature parity (ignore, tokens, secrets, tree-sitter, comment removal, git history, drag-and-drop, GitHub URL) → Phases 1-3 cover ignore/tokens/secrets/tree-sitter/protocol/orchestrator. **Comment removal, git history, drag-and-drop, and GitHub URL packing are deferred** to follow-up plans — they're not blockers for v0.1.0 and the orchestrator/UI can call into them once they exist. Note: `core::github::parse_github_url` and `shallow_clone` *are* implemented in Task 2.4 — they just aren't wired through the orchestrator. The pack_start command returns a clear "not implemented in v0.1.0" error for GitHub targets.
- MSI + portable .exe ship → Task 8.1, 8.2
- No CLI, no macOS/Linux, no signing, no auto-update, no telemetry → respected throughout

Open follow-up tasks (won't be in v0.1.0):
- **GitHub URL packing wiring** (orchestrator clones the repo, drops temp dir on completion, cancels mid-clone). Highest-priority follow-up — likely v0.2.0.
- **Comment removal** (`core::comments` module). Follow-up plan.
- **Git history support** (`core::git_history` module). Follow-up plan.
- **Drag-and-drop** folder onto the window. Follow-up plan.
- **Encoding fallback** for non-UTF-8 (only UTF-16LE fallback exists today; spec §9.3 calls for Windows-1252 too). Follow-up plan.
- **Better error plumbing** through the pack pipeline (currently the failure path is silent — UI has to time-out instead of receiving an error event). Follow-up plan.
- **E2E Playwright test**. Optional add to CI.
- **Property tests** for ignore/walker. Optional add.
- **Frontend vitest tests**. Skipped per spec §10.7.

These deferrals keep v0.1.0 focused on the new and most valuable thing: the Grok ↔ Claude Code round-trip.
