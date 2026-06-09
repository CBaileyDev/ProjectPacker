# Reddit launch posts

> **Embargo: do not post either thread until the linked release has binaries for Windows, macOS, and Linux.**

Post the r/rust thread first; if it lands well, post r/LocalLLaMA the next day (cross-posting both simultaneously reads as spam).

---

## r/rust

**Title:**

```
ProjectPacker: a Tauri desktop app that packs repos for AI workflows — pure-Rust core with gix, tree-sitter, and a plan validator (MIT)
```

**Body:**

```
I've been building a desktop tool for the "web AI plans, local agent executes"
workflow and the Rust side turned out interesting enough to share.

**The core is a pure-Rust library crate** (no Tauri dependency) that does the
whole pipeline: parallel file walk → 3-tier ignore stacking (builtin defaults,
.gitignore, user file) → secret scan and redaction → content transforms →
tokenize → emit. Highlights:

- **gix** for shallow-cloning GitHub URLs — no git binary dependency.
- **tree-sitter** for skeleton compression: signatures and types survive,
  function bodies get elided. Compiled queries are cached per-language in a
  OnceLock (was recompiling per file — 1000 redundant compiles on a 500-file
  repo).
- **Vendored gitleaks v8.25.0 ruleset** (~167 rules) running on a hand-rolled
  engine: Aho-Corasick keyword pre-filter, Shannon-entropy gating,
  specificity-aware overlap resolution. ~200µs warmed scan on a 100KB file.
- **Vendored HuggingFace tokenizers** (pure-Rust regex backend, no onig) for
  real per-model token counts: GPT-4o, Llama 3, Qwen 2.5, DeepSeek, Mistral.
- **rayon** across the hot loops — secret scan, tokenize, and pin-reorder are
  all parallel; emit buffers are pre-allocated from byte totals.
- **specta + tauri-specta** generate the TypeScript bindings, so the React
  frontend gets compile-checked types for every command and event.
- The protocol layer (the actual point of the app — a strict plan format that
  gets validated before a coding agent executes it) is a ~250-line
  hand-rolled markdown validator with insta golden tests freezing the
  protocol per version.

Design doc with the full architecture (walker → transforms → emit dataflow,
tier-stacking semantics, protocol freezing policy):
https://github.com/CBaileyDev/ProjectPacker/blob/main/docs/superpowers/specs/2026-04-30-projectpacker-design.md

MIT, binaries for all three platforms on the releases page.
https://github.com/CBaileyDev/ProjectPacker

Happy to go deep on any of the above — the gitleaks engine and the
tree-sitter query caching were the most fun to get right.
```

---

## r/LocalLLaMA

**Title:**

```
Free, offline desktop tool: pack any repo for your AI + validate the plan it sends back. No API keys, no telemetry, BYO models (MIT)
```

**Body:**

```
Built this for my own workflow and figured this sub would appreciate the
constraints it was built under:

- **Fully offline.** The only network call is the GitHub clone you explicitly
  ask for. No accounts, no API keys, no telemetry, no update pings.
- **BYO AI.** It produces text you paste into whatever you run — a local
  model, a web UI, an agent. Nothing is locked to a provider.
- **Real tokenizers, locally.** Vendored HF tokenizers for Llama 3, Qwen 2.5,
  DeepSeek, Mistral, GPT-4o — so the token count you see is the token count
  your model sees, computed on your machine.
- **Secret scanning before anything leaves your machine.** gitleaks ruleset,
  ~167 rules, redacts in place and lists every redaction in the output.

The differentiating bit: it doesn't just pack. The pack embeds a strict plan
protocol, and the app validates the plan your AI writes *before* you hand it
to an executor agent. Malformed plan → one-click re-prompt naming each
violation. Valid plan → combined prompt wrapping it in executor instructions
with explicit veto power. Works with local planners too — smaller models
actually benefit most from the re-prompt loop, since they drift from the
format more often.

Rust + Tauri, so the binary is small and fast. MIT. Windows/macOS/Linux
binaries on the releases page (unsigned, sha256 hashes published).

https://github.com/CBaileyDev/ProjectPacker
```

## Engagement notes

- r/rust will ask about the hand-rolled secrets engine vs. using gitleaks as a lib (it's Go — that's the answer) and about `unsafe` count (zero in the workspace; say so).
- r/LocalLLaMA will ask for a CLI — it's first on the roadmap; say that and link the roadmap section.
- Don't post version numbers in titles; threads age better without them.
