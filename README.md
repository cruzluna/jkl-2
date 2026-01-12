# jkl2

A CLI/TUI tool for inspecting tmux sessions.

## Requirements

- `tmux`
- `fzf`

## Install

```
cargo install --git https://github.com/cruzluna/jkl-2
```

If needed, ensure `~/.cargo/bin` is on your `PATH`.

## Usage

- Launch the TUI: `jkl2 tui`
- Quit the TUI: `q`, `Esc`, or `Ctrl+C` (Ctrl+C exits search first)
- Navigate rows: `↑`/`↓` or `j`/`k`
- Search sessions: `/` (type to filter, `Esc` to exit search)
- Switch to session: `Enter`
- Upsert session metadata: `jkl2 upsert <session_id> <session_name> [--status <status>] [--context <text>]`

## Tmux Plugin (TPM)

Add the plugin and reload TPM:

```
set -g @plugin 'cruzluna/jkl-2'

# Initialize TMUX plugin manager (keep at bottom)
run '~/.tmux/plugins/tpm/tpm'
```

Default prefix bindings:

- `f`: open `jkl tui` in a popup
- `c`: prompt for context and run `jkl upsert '#S' '#S' --context <input>`
- `e`: open `~/.config/jkl/session_context.json` in `nvim`

## Session Context

The TUI reads optional metadata from `~/.config/jkl/session_context.json`. If the file does not exist, it is created with `{}` the first time you run the TUI.

Shape (keyed by `session_id`):

```json
{
  "$1": {
    "session_name": "work",
    "status": "idle",
    "context": "my project"
  }
}
```

Upsert example:

```
jkl2 upsert "$1" "work" --status working --context "my project"
```

Status values:

- `working` (blue)
- `waiting` or `idle` (yellow)
- `done` (green)
- missing values render as `-`

## Testing

- `cargo check`
- `cargo test`

## Development

- Run TUI locally: `cargo run -- tui`
- Point tmux at a test server: `tmux -L test list-sessions`
- Use a temp context file: `HOME=/tmp/jkl-dev cargo run -- tui`
