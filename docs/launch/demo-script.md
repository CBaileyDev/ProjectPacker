# Hero GIF recording script

Target: `docs/assets/hero.gif`, **under 45 seconds**, 1280px wide, dark theme (the app's default). Record at 2× display scale if possible so text stays crisp after GIF compression. No audio. Keep the cursor deliberate — every move should telegraph the next click.

The validation-failure moment (step 5) is the money shot. Everything before it is setup; don't linger.

## Beats

| t | Action |
|---|---|
| 0–4s | App open on the Packer tab. Drag a real repo folder from Explorer/Finder onto the window. Target path fills in. |
| 4–10s | Click **Pack**. Let the progress bar and live log run. Don't skip this — live progress reads as "real app, real work". |
| 10–16s | Results tab auto-opens. Hover the stats row (files, tokens per model, duration). Click **Copy Pack XML** — let the button flip to "Copied!". |
| 16–18s | Click the **Bridge** tab. |
| 18–28s | Paste the **malformed plan** (below, copy it to clipboard before recording). Click **Validate plan**. Red error panel renders with named violations and the **Copy re-prompt** button. **Pause two beats here** — this is the frame people screenshot. |
| 28–38s | Select-all in the textarea, paste the **fixed plan** (below). Click **Validate plan**. Green "Plan is valid" panel + **Copy combined prompt** button appear. |
| 38–43s | Click **Copy combined prompt** → "Copied!". End on that state. |

## Malformed plan (paste at 18s)

Three violations: missing Risks section, Step 1 has no Rationale, Verification has no bullets.

```markdown
### Summary
Add a health-check endpoint to the API server.

### Steps

#### Step 1: Add the endpoint
**Action:** edit
**Target:** src/server.rs
**Details:**
Add a GET /healthz route returning 200.

### Verification

### Rollback
- Use git to revert.
```

Expected validator output: `missing_section` (Risks), `missing_field` (Step 1 Rationale), `verification_empty`.

## Fixed plan (paste at 28s)

```markdown
### Summary
Add a health-check endpoint to the API server.

### Risks
- None.

### Steps

#### Step 1: Add the endpoint
**Action:** edit
**Target:** src/server.rs
**Rationale:** Load balancers need an unauthenticated liveness probe; no such route exists today.
**Details:**
Add a GET /healthz route returning 200 with body "ok".

### Verification
- `cargo test` passes.
- `curl localhost:8080/healthz` returns 200.

### Rollback
- Use git to revert.
```

## After recording

1. Export as GIF (gifski or similar; target < 8 MB so the README loads fast).
2. Save to `docs/assets/hero.gif`, commit.
3. In `README.md`, replace the `<!-- hero.gif: ... -->` comment block with `![ProjectPacker demo](docs/assets/hero.gif)`.
