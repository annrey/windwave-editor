# Repository Boundaries

> Updated: 2026-07-10  
> Purpose: clarify that **this repository is WindWave-only** and does not ship Understand Anything or other co-located workspaces.

## What This Repo Is

**windwave-editor** is the canonical home for WindWave (`agent-edit`): a Rust + Bevy AI Agent driven game editor.

Primary source of truth:

- `Cargo.toml`, `Cargo.lock`
- `src/main.rs`
- `crates/`
- `CONTEXT-MAP.md`, `PROJECT_STATUS.md`, `CURRENT_CAPABILITIES.md`
- `docs/project-situation.md`
- `docs/windwave-version-plan.md`, `docs/windwave-execution-plan.md`
- `docs/remaining-work.md`

Quality gates:

```bash
cargo build
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

See also `REPO_LAYOUT.md` at the repository root for crate roles and doc entry points.

## What Is Intentionally Out of Tree

These are **not** part of this repository and must not be copied here:

| Item | Where it lives |
|------|----------------|
| Understand Anything (Node plugin, pnpm workspace) | Sibling mixed workspace `../风浪` and remote [Lum1104/Understand-Anything](https://github.com/Lum1104/Understand-Anything.git) |
| Multica server vendor tree (`multica/`, ~1.2G) | External / local-only; integrate via `crates/multica-bridge` |
| Session logs, private design folders, screenshots | Local only (see `.gitignore`) |
| Node tooling (`package.json`, `pnpm-*`, plugin marketplaces) | Understand Anything workspace, not WindWave |

Agents and contributors should **not** assume `pnpm`, Vite, or `understand-anything-plugin/` exist in this repo. If documentation in `docs/archive/` mentions a co-located UA workspace, treat that as historical context from the old mixed tree.

## Documentation Order (WindWave)

1. `docs/project-situation.md` — current status and P0/P1/P2 queue  
2. `docs/windwave-version-plan.md` — version goals and acceptance gates  
3. `docs/windwave-execution-plan.md` — code-level implementation steps  
4. `PROJECT_STATUS.md` — historical status and quality-gate notes  
5. `docs/remaining-work.md` — open engineering items  

Historical Multica integration notes live under `docs/archive/multica/`. Files under `docs/deprecated/` (if present) are reference only.

## Quick Heuristic

- **Cargo**, `src/`, `crates/` → WindWave (this repo).  
- **pnpm**, `understand-anything-plugin/`, Astro/Vite → Understand Anything (elsewhere).
