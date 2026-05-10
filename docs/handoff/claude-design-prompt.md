# Claude Design — ProjectPacker UI handoff prompt

> **Usage:** Open this file, copy the prompt block below, paste it into a new Claude Design conversation. As components arrive, save them under `frontend/src/components/` or replace `frontend/src/routes/Pack.tsx` and run `pnpm tauri dev` to see them live.

---

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

These are the auto-generated TypeScript types from `frontend/src/bindings/index.ts`. Components must accept props matching these shapes exactly.

```ts
export type AppError = { code: string; message: string; details: string | null }
export type FileFound = { path: string; bytes: number }
export type GoalTemplate = { name: string; body: string }
export type PackOptions = { target: PackTarget; goal: string; includeGitHistory: boolean; countTokens: boolean; tokenizerModel: string; secretScan: boolean; compress: boolean; removeComments: boolean; maxFileSizeKb: number; respectGitignore: boolean; customIgnorePatterns: string[]; protocolVersion: string }
export type PackResult = { xml: string; claudeCodePrompt: string; stats: PackStats; warnings: PackWarning[] }
export type PackStats = { filesTotal: number; filesIncluded: number; filesSkipped: number; bytesTotal: number; tokensTotal: number | null; secretsFound: number; durationMs: number }
export type PackTarget = { kind: "folder"; value: string } | { kind: "github"; value: string }
export type PackWarning = { kind: WarningKind; path: string | null; message: string }
export type PlanError = { code: string; message: string }
export type PlanValidation = { ok: boolean; errors: PlanError[] }
export type Preset = { name: string; optionsJson: string }
export type ProgressEvent =
  | { kind: "started"; job_id: string; target_label: string }
  | { kind: "cloning"; progress_pct: number }
  | { kind: "walking"; files_scanned: number }
  | { kind: "fileFoundBatch"; paths: FileFound[] }
  | { kind: "fileSkipped"; path: string; reason: SkipReason }
  | { kind: "tokenizing"; progress_pct: number }
  | { kind: "secretScanning"; progress_pct: number }
  | { kind: "secretHit"; path: string; secret_kind: string; line: number }
  | { kind: "compressing"; progress_pct: number }
  | { kind: "cloning"; progress_pct: number }
  | { kind: "buildingOutput" }
  | { kind: "done"; stats: PackStats }
  | { kind: "error"; message: string; fatal: boolean }
export type Recent = { label: string; target: string; lastUsedIso: string }
export type Settings = { theme: Theme; defaultProtocolVersion: string; defaultTokenizerModel: string; recents: Recent[]; goalTemplates: GoalTemplate[]; presets: Preset[] }
export type SkipReason = { kind: "ignored" } | { kind: "tooLarge" } | { kind: "binary" } | { kind: "inaccessible" } | { kind: "encodingFailed" }
export type Theme = "dark" | "light"
export type WarningKind = { kind: "fileSkipped" } | { kind: "treeSitterFailed" } | { kind: "gitLogMissing" } | { kind: "encodingFallback" } | { kind: "secretScanFailed" }
```

The available commands are typed wrappers around Tauri invokes:

```ts
commands.packStart(opts: PackOptions): Promise<Result<string, AppError>>           // returns jobId
commands.packCancel(jobId: string): Promise<Result<null, AppError>>
commands.packGetResult(jobId: string): Promise<Result<PackResult, AppError>>
commands.validatePlan(planMd: string, protocolVersion: string): Promise<Result<PlanValidation, AppError>>
commands.buildCombinedPrompt(planMd: string, protocolVersion: string): Promise<Result<string, AppError>>
commands.getSettings(): Promise<Result<Settings, AppError>>
commands.saveSettings(settings: Settings): Promise<Result<Settings, AppError>>
commands.saveToFile(path: string, contents: string): Promise<Result<null, AppError>>
```

Pack progress arrives via Tauri events on topic `pack:{jobId}:progress` carrying `ProgressEvent` payloads. Subscribe with `subscribePackProgress(jobId, onEvent)` from `frontend/src/lib/events.ts`.

## Components to build

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

## Constraints

- All components live under `frontend/src/components/` and `frontend/src/routes/`.
- No external network calls — the app is fully offline.
- Components should accept their data via typed props matching the
  bindings/ types; do not invent new shapes.
- Animations should be tasteful, never block input, never run >2s.
- Accessibility: keyboard navigation everywhere, focus rings visible,
  ARIA labels on icon-only buttons.

## What I want back

Code. Drop-in `.tsx` files for each route and component, with all CSS
inline as Tailwind classes. Use Framer Motion. Use lucide-react for
icons. Match the typed props from the bindings exactly.

When you have a draft, show me the Pack route first — that is the visual
centerpiece of the app.
