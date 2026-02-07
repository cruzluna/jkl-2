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

