# WindWave Development Environment

> Updated: 2026-06-07

## Source Of Truth

WindWave is a Rust workspace. Use Cargo as the only source of truth for build,
test, features, and dependency resolution.

Primary entrypoints:

- `Cargo.toml`
- `Cargo.lock`
- `src/main.rs`
- `crates/`

Co-located Node/Understand Anything files such as `package.json`, `pnpm-*`,
`understand-anything-plugin/`, `homepage/`, and `docs/superpowers/` are not the
WindWave runtime or quality gate. See `docs/repository-boundaries.md`.

## Required Tools

- Rust toolchain with Cargo
- macOS windowing support for Bevy/winit
- Optional: `rust-analyzer` in VS Code or Cursor
- Optional: Xcode, only as a source browser or external launcher

## Daily Commands

Use the root `Makefile` as the shared local entrypoint:

```bash
make run
make check
make test
make clippy
make gate
```

Equivalent raw Cargo commands:

```bash
cargo run --bin agent-edit
cargo check
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## P0/P1 Smoke

Run the focused closed-loop suite:

```bash
make smoke-p0-p1
```

This covers:

- Red enemy closed-loop execution and undo
- HR approval approve/reject smoke through main-app systems
- EngineCommand reverse contracts
- SceneIndex deleted-entity cleanup
- Failed-plan revision event emission

## IDE Guidance

Recommended day-to-day IDE:

- VS Code or Cursor with `rust-analyzer`

Xcode:

- Useful for browsing files on macOS.
- Not the real build system.
- This repository has no native Xcode scheme for Cargo.
- Use `make open-xcode` to open the folder, then run Cargo from terminal or the
  Makefile.

## Runtime Notes

`cargo run --bin agent-edit` starts the Bevy editor. Bevy `png` support is
enabled in `Cargo.toml` so the screenshot pipeline can save PNG output.

Computer Use currently does not expose the Bevy/winit editor window as a
controllable macOS app. Use automated smoke tests as the proxy signal, then do
final window-level UI checks manually when needed.

## iCloud Checkout Notes

This checkout lives under iCloud Drive. Cargo currently works here, but iCloud
can occasionally introduce file-lock or sync latency. If builds become flaky,
copy the workspace to a normal local directory or set a temporary target dir:

```bash
CARGO_TARGET_DIR=/private/tmp/windwave-target cargo test --workspace
```
