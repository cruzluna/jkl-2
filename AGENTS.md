# [AGENTS.md](http://AGENTS.md)

Use this as the default playbook for coding agents in this repository.

## Scope

- Project: Rust CLI/TUI app (`jkl`) for tmux session/pane status tracking.
- Entry point: `src/main.rs`.
- Important modules:
- `src/cli.rs` command parsing and dispatch.
- `src/context.rs` JSON persistence and sync logic.
- `src/tmux.rs` tmux command integration.
- `src/tui.rs` Ratatui interface and pane selector.

## Build, Test, Lint

- Build:`cargo build`
- Run:
- `cargo run -- tui`
- `cargo run -- upsert my session --status working`
- Test all:
- `cargo test --locked`
- Test one module:
- `cargo test tui::tests`
- Test one test:
- `cargo test handle_upsert_rejects_invalid_status -- --exact`
- Test one test with output:
- `cargo test handle_upsert_rejects_invalid_status -- --exact --nocapture`
- Format:
- `cargo fmt --all`
- `cargo fmt --all -- --check`
- Clippy:
- `cargo clippy --all-targets`
- `cargo clippy --all-targets -- -D warnings`

## Agent Expectations

- Make minimal, targeted changes.
- Preserve existing CLI behavior and keybindings unless requested.
- Update docs when commands or behavior change.
- Defer to runtime tmux values for targeting and navigation. Session/window/pane IDs can change after tmux resurrect or restart.
- Treat persisted context IDs as advisory metadata. Re-resolve live IDs from tmux (`list-sessions`, `list-panes`, pane targets) before executing switch/select commands.
- Use conventional commit messages

## Cursor Cloud specific instructions

- The project uses Rust edition 2024 (`Cargo.toml`), which requires Rust >= 1.85.0. The update script runs `rustup update stable` to keep the toolchain current.
- `cargo clippy --all-targets -- -D warnings` produces pre-existing warnings on Rust >= 1.95 (`unnecessary_get_then_check`). These are not regressions; `cargo clippy --all-targets` (without `-D warnings`) is the practical lint check.
- The TUI (`cargo run -- tui`) requires a tmux server with at least one session. Start a tmux session before launching the TUI, or tests that depend on live tmux will fail.
- Tests use `EnvGuard` with temp directories and a fake tmux script, so `cargo test --locked` works without a running tmux server.
- Session metadata is stored at `~/.config/jkl/session_context.json` (created automatically on first run).
