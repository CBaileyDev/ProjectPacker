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

From the repo root:

```bash
pnpm install
pnpm tauri dev
```

Other useful scripts (run from the repo root):

```bash
pnpm tauri build --debug --no-bundle    # build the desktop app without packaging
pnpm bindings                           # regenerate specta TypeScript bindings
```

## License

MIT — see [LICENSE](LICENSE).
