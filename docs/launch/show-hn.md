# Show HN post

> **Embargo: do not post until the linked release has binaries for Windows, macOS, and Linux.**

## Title

```
Show HN: I built a validated handoff protocol between a planning AI and a coding agent
```

(78 chars — fits. Alternative if a mod asks for the project name: `Show HN: ProjectPacker – validate an AI's plan before your coding agent runs it`)

## URL

```
https://github.com/CBaileyDev/ProjectPacker
```

## First comment (post immediately after submitting)

```
Repo packers are a solved problem — Repomix, gitingest, and code2prompt are all
good and free, and my pack output is deliberately compatible with that
ecosystem. So this isn't "another repo packer".

What I couldn't find anywhere: validation of what the AI sends *back*. My
workflow is plan-in-a-web-AI (big context, cheap iterations), execute-with-a-
local-agent (file access, can run tests). The failure mode is always the
handoff — the planner hallucinates a file path or buries something destructive
in step 7, and the executor just... does it.

ProjectPacker embeds a versioned protocol (plan-exec-v1) in every pack that
forces the planner to emit a strict-format plan: five mandatory sections, an
Action/Target/Rationale/Details block per step, rationale required on every
step. A validator checks the plan before it reaches your agent. Malformed plans
get a one-click re-prompt naming each violation — paste it back and the planner
fixes itself, usually in one round. Valid plans get wrapped in an executor
prompt that tells the agent to challenge any step whose rationale doesn't hold
up against the actual repo — explicit veto power, not blind execution.

Protocol versions are frozen at release; improvements ship as v2 rather than
silently changing what "valid" means.

Tech: Rust core (gix for cloning, tree-sitter for skeleton compression,
vendored gitleaks rules for secret redaction, vendored HF tokenizers for
per-model token counts), Tauri 2 shell, React frontend. Fully offline — the
only network call is the GitHub clone you explicitly ask for. No accounts, no
API keys, no telemetry. MIT.

Binaries for Windows/macOS/Linux are on the releases page (unsigned — sha256
hashes published, SmartScreen/Gatekeeper notes in the release notes).

Happy to answer questions about the protocol design — especially interested in
whether the strict-format approach holds up against other people's planner
models.
```

## Engagement notes

- Expected pushback: "why not just use a better agent?" — answer honestly: agents are getting better at self-review, but a contract at the boundary is model-agnostic and survives model churn. The protocol works with whatever planner/executor exists next year.
- Expected pushback: "the validator only checks format, not correctness" — concede immediately, it's true. Format validation catches the cheap failures (missing rationales, malformed steps) mechanically; semantic review is the executor's job, and the protocol's role there is forcing rationales to exist so the executor has something to evaluate.
- Do not argue with "Repomix does packing better" comments. Agree, link the comparison table.
