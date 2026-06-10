# ProjectPacker UI Redesign — "Four Moments, Graphite & Ember"

**Date:** 2026-06-09
**Status:** Approved by maintainer (structure + theme chosen via visual companion session)
**Scope:** Frontend only. Zero Rust/IPC changes — every screen consumes the exact commands, events, and bindings that exist today.

## Summary

Replace the 5-tab sidebar app with a single window that morphs through four
moments matching the plan-exec protocol journey: **Home → Packing → Results
→ Bridge**. Apply a new visual identity — **Graphite & Ember**: warm
charcoal surfaces, one ember-orange brand accent, with green and red
reserved exclusively for semantic status (validation passed / secrets
caught). The Pack.tsx monolith (1,179 lines) decomposes into one component
per moment.

## Goals

- The app's structure tells the product story: pack → hand to planner →
  validate plan → hand to executor.
- Repeat packs are faster than today (drop folder / pick recent → one click).
- The app looks like a tool people screenshot: Raycast/Linear-grade polish.
- Brand color never collides with status color.

## Non-goals

- No Rust, IPC, command, or binding changes.
- No new packing features, options, or formats.
- No README/screenshot refresh (separate task after implementation).
- No full command palette (only the ⌘K focus shortcut described below).

## Information architecture

### Moments (replaces `PackTab`)

The zustand store's `activeTab: PackTab` becomes a `moment` state machine:

```
moment: "home" | "packing" | "results" | "bridge"
```

Transitions:

| From | Event | To |
|---|---|---|
| home | pack started | packing |
| packing | `done` event | results |
| packing | cancel or fatal `error` event | home (error surfaces as toast; log preserved until next pack) |
| results | "Open Bridge" | bridge |
| bridge | back | results (or home if no result in memory) |
| results | "New pack" / logo | home (options retained) |
| any | logo click | home |

Always boot into `home` (results are in-memory only today; nothing to
restore). `moment` is **not** persisted.

GitHub and Settings are not moments — they are overlay sheets:

```
activeSheet: "github" | "settings" | null
```

opened from persistent title-bar icons, closable with Esc.

### Moment contents

**Home** (`routes/Home.tsx`)
- Protocol badge (`plan-exec-v1`, mono, ember) above an oversized headline:
  "What are we packing?"
- Drop zone (existing `use-drag-drop` + `DropOverlay`) + GitHub URL/local
  path field with inline **Pack ↵** button. Enter packs.
- Preset chips: **Balanced / Minimal tokens / Everything / Custom…** —
  defined in a new `lib/presets.ts` as bundles over existing `PackOptions`
  fields (compression toggles, maxFileSizeKb, etc.). Selecting a chip
  patches options; "Custom…" (and any manual divergence from a preset)
  opens/marks the Advanced panel.
- Recents row from existing persisted recents.
- "Advanced options" disclosure: the full current option set (transforms,
  secret scan, format, xml schema, tokenizer, ignores) — i.e. today's
  options panel content, collapsed by default, state persisted.
- ⌘K focuses the target field from anywhere (and returns to Home first if
  needed). Implemented in existing `use-keyboard-shortcuts`.

**Packing** (`routes/Packing.tsx`)
- Full-screen progress theater reusing existing event stream:
  `PackProgressBar` (phase timeline), `PhaseBreakdown`, `ProgressLog`
  (live log), transform progress, secret-scan counters.
- One action: Cancel (existing `pack_cancel`).

**Results** (`routes/Results.tsx`)
- Crumb line: target · format · duration · "← new pack".
- Stats hero: token total (large, mono, ember em), files included,
  redactions; savings line from `CompressionPanel` data.
- "Fits in" card: derived from existing `tokensPerModel` plus a static
  context-window table in `lib/model-windows.ts`
  (`{ model, windowTokens }`); states: ✓ fits / ◐ tight (>75%) / ✗ over.
- Output card: **Copy pack for planner** (primary, ember), Save to file,
  Re-pack with tweaks (returns Home with options intact).
- `CompressionPanel` (transform savings detail) below, collapsed.
- Bridge banner: "Got a plan back from your planner?" → Open Bridge.

**Bridge** (`routes/Bridge.tsx`)
- Existing `BridgeTab` functionality restyled: paste plan → validate
  (`validate_plan`) → violations list (red = fail, green = pass — semantic
  colors, not brand) → copy combined executor prompt
  (`build_combined_prompt`) or copy re-prompt.

### Shell (`App.tsx`)
- Sidebar deleted. New title-bar strip: traffic-light inset spacing,
  wordmark, right side: ⌘K hint, GitHub status dot (connected = ember,
  not = dim), settings gear. Sheets render over a scrim (`z-modal`).
- Moment container renders the active moment with framer-motion
  cross-fade + 12px vertical slide (`fadeUp` family), honoring
  `prefersReducedMotion` exactly as `lib/motion.ts` does today.

## Visual identity — Graphite & Ember

All values land in `globals.css` (existing oklch token system, same token
names — extended, not replaced) and remain theme-pair aware.

Dark (default):
- `--background`: warm graphite `oklch(0.16 0.005 75)` (~#0d0e10)
- Ambient gradient: ember radial top (`rgba(245,158,66,0.07)`), faint
  violet bottom-right (unchanged structure, recolored)
- Panels: `rgba(255,255,255,0.035)` fills, `rgba(255,255,255,0.07)` borders,
  radius 12–16px, soft inner top highlight
- `--primary` (brand/ember): `oklch(0.78 0.14 70)` (~#f59e42),
  `--primary-foreground` near-black `oklch(0.2 0.04 70)`
- `--ring` / focus / glow: ember at 25% alpha (replaces emerald glow)
- Status (reserved, never used for brand): `--success` green ~#22c55e
  (validation passed, fits), `--destructive` red ~#ef4444 (validation
  failed, secrets), `--warning` amber distinct from ember by luminance
  (tight fits), `--info` unchanged
- `--transform-savings` recolored to the success green family

Light: warm paper `oklch(0.97 0.005 85)`, same ember accent, same
semantics. Theme switch continues to work via existing `.dark` class.

Typography: system stacks (no new font dependency — keeps the bundle
hermetic). All numerals tabular via the existing `nums` utility; stats use
mono. Headline weight 700, tracking −0.02em.

## Component disposition

| Existing | Fate |
|---|---|
| `Pack.tsx` (1,179 lines) | split into `Home/Packing/Results/Bridge` routes + shared bits; deleted |
| `App.tsx` sidebar | replaced by title-bar shell + sheets |
| `Settings.tsx` | split: pack options → Home Advanced panel; app prefs (theme, defaults) → Settings sheet |
| `GithubConnector.tsx` | unchanged logic, rendered in GitHub sheet |
| `BridgeTab.tsx` | becomes Bridge moment body (tests keep passing) |
| `ProgressLog`, `PackProgressBar`, `PhaseBreakdown` | Packing moment, restyled |
| `StatsBar`, `AiContextTable` | merged into Results hero + Fits-in card |
| `CompressionPanel`, `TransformRow` | Results, collapsed section (tests keep passing) |
| `CopyButton`, `SaveButton`, `Toggle`, `DropOverlay`, toasts | restyled tokens only |

## Data flow & state

- Store keeps: `options`, `status`, `events`, `lastStats`, `result` access
  via `use-pack-job` — unchanged.
- `activeTab` → `moment` + `activeSheet` (rename + state machine above);
  `status` transitions drive automatic moment changes (started → packing,
  done → results, error/cancel → home).
- New `lib/presets.ts` (preset definitions + `applyPreset`,
  `matchPreset(options)` for chip highlighting) and `lib/model-windows.ts`
  (static table + `fitsIn(tokens)` helper) — both pure, both unit-tested.
- Persistence: same store file, same sanitize moat. Persisted additions:
  `advancedOpen: boolean`, `activePreset: string | null`. Sanitize
  whitelist extended accordingly.

## Error handling

- Pack fatal error: toast (existing toast system) + return to Home;
  ProgressLog content retained and viewable via a "last run" link on Home
  until the next pack starts.
- Bridge validation errors are content, not failures (red list items).
- Sheets trap focus, close on Esc; drop-zone rejects non-directories with
  the existing toast path.

## Testing

- Unit (vitest): moment state-machine transitions (all rows of the table
  above), `presets` apply/match round-trip, `fitsIn` boundaries
  (fits/tight/over), sanitize accepts the two new persisted keys.
- Existing component tests (`BridgeTab`, `CompressionPanel`,
  `pack-progress`, `paginate`, `persist-adapter`) must pass unmodified or
  with import-path-only updates.
- Gate: `pnpm typecheck` + full vitest + `cargo test --workspace`
  (unchanged Rust must stay green) before merge.

## Implementation notes

- Branch: continues on `refactor/simplification-pass` (stacked on the
  simplification + rehydration fix).
- The Pack.tsx decomposition lands first as a pure restructure (moments
  rendering existing visuals), then the theme/token pass, then per-moment
  polish — so every commit keeps the app working.
