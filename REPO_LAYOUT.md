# WindWave Repository Layout

This repo contains only the WindWave Rust workspace (AI Agent game editor). Vendor servers and the Understand Anything plugin live outside the tree.

## Crates (`crates/`)

| Crate | Role |
|-------|------|
| **agent-core** | Agent runtime: planning, memory, tool execution, and editor-facing agent APIs shared across UI and simulation. |
| **agent-ui** | Bevy UI layers, panels, and interaction glue between the human editor and agent-core. |
| **bevy-adapter** | Bevy engine integration: scenes, entities, and editor hooks abstracted for agent operations. |
| **multica-bridge** | Client bridge to Multica-style multi-agent services (HTTP/API); no bundled Multica server vendor. |
| **ai-frameworks** | Adapters and shared types for external LLM / agent frameworks used by the editor. |
| **game-simulator** | Headless or lightweight simulation for testing agent behavior without full editor chrome. |

## Application entry

- **`src/main.rs`** — binary entry for the editor (`agent-edit` workspace root in `Cargo.toml`).

## Templates

- **`templates/`** — starter assets or project scaffolds used by the editor.

## Documentation entry points

| Doc | Use |
|-----|-----|
| `docs/remaining-work.md` | Open tasks and engineering backlog |
| `docs/windwave-version-plan.md` | Version targets and release gates |
| `docs/windwave-execution-plan.md` | Implementation sequencing |
| `CONTEXT-MAP.md` | Domain vocabulary and subsystem map |
| `docs/project-situation.md` | Current program status |
| `docs/repository-boundaries.md` | What belongs in this repo vs sibling workspaces |

Archived deep dives: `docs/archive/` (including `docs/archive/multica/` for legacy Multica integration write-ups).

## Build

```bash
make        # if targets are defined
cargo build
cargo test --workspace
```

## Out of tree (by design)

- **`multica/`** vendor server — not shipped; configure endpoints for `multica-bridge` locally.
- **Understand Anything** — sibling folder `../风浪` and [Understand-Anything](https://github.com/Lum1104/Understand-Anything.git); not co-located in this repository.
