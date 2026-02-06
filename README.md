# jkl

A CLI/TUI tool for inspecting tmux sessions.

## Requirements

- `tmux`
- `fzf`

## Install

### Binary (recommended)

```
curl -fsSL https://raw.githubusercontent.com/cruzluna/jkl-2/main/install.sh | bash
```

Optional: set `JKL_INSTALL_DIR` to choose where the binary is installed.

### Cargo

```
cargo install --git https://github.com/cruzluna/jkl-2
```

If needed, ensure `~/.cargo/bin` is on your `PATH`.

## Update

```
jkl update
```

To include pre-releases:

```
jkl update --prerelease
```

Pre-releases are selected from tags that include `-rc.` (for example, `v0.2.0-rc.1`).

If you installed via Cargo:

```
cargo install --git https://github.com/cruzluna/jkl-2 --force
```

### Release assets

The update command and install script expect GitHub release assets named:

- `jkl-x86_64-apple-darwin.tar.gz`
- `jkl-aarch64-apple-darwin.tar.gz`
- `jkl-x86_64-unknown-linux-gnu.tar.gz`
- `jkl-aarch64-unknown-linux-gnu.tar.gz`

Each archive should contain the `jkl` binary at the top level.

## Usage

- Launch the TUI: `jkl tui`
- Quit the TUI: `q`, `Esc`, or `Ctrl+C` (Ctrl+C exits search first)
- Navigate rows: `↑`/`↓` or `j`/`k`
- Expand/collapse panes: `l`/`h`
- Refresh pane list: `r`
- Search sessions: `/` (type to filter, `Esc` to exit search)
- Switch to session: `Enter`
- Upsert session metadata: `jkl upsert <session_name...> [--session-id <session_id>] [--status <status>] [--context <text...>]`
- Upsert pane metadata: `jkl upsert <session_name...> --pane-id <pane_id> [--status <status>] [--context <text...>]`
- Rename session entry: `jkl rename <session_id> <session_name...>`
- Pane status selector: `jkl tui --pane-state --session-name <session_name...> --pane-id <pane_id>`

Multi-word session names or context can be passed without quotes; use `--` to terminate positional values if needed.

## Tmux Plugin (TPM)

Add the plugin and reload TPM:

```
set -g @plugin 'cruzluna/jkl-2'

# Initialize TMUX plugin manager (keep at bottom)
run '~/.tmux/plugins/tpm/tpm'
```

If tpm fails to download plugin: 
``` bash
$ tmux run-shell "~/.tmux/plugins/tpm/bin/install_plugins"
```

Default prefix bindings (only set when the key is currently unbound):

- `f`: open `jkl tui` in a popup
- `c`: prompt for context and run `jkl upsert '#S' --session-id '#{session_id}' --context <input>`
- `e`: open `~/.config/jkl/session_context.json` in `nvim`
- `S`: open pane status selector popup

You can configure or disable each key:

```tmux
# Set custom keys
set -g @jkl_key_tui 'J'
set -g @jkl_key_context 'C'
set -g @jkl_key_edit 'E'
set -g @jkl_key_pane_state 'P'

# Disable a binding
set -g @jkl_key_edit 'none'
```

By default the plugin does not override existing tmux/user bindings. If you want it to force overrides, set:

```tmux
set -g @jkl_force_bind_keys 'on'
```

## Session Context

The TUI reads optional metadata from `~/.config/jkl/session_context.json`. If the file does not exist, it is created with `{}` the first time you run the TUI.

Shape (keyed by `blake3(session_name)`):

```json
{
  "2f0d7b3b5e3b9b1d4b4b5b8b8e2e2e9a2d2d4d5f2f0f5e5f2d9b3f1a5c8e": {
    "session_name": "work",
    "status": "idle",
    "context": "my project",
    "panes": {
      "%1": {
        "status": "working",
        "context": "focus time"
      }
    }
  }
}
```

Upsert examples:

```
jkl upsert "work" --status working --context "my project"
jkl upsert "work" --pane-id %1 --status working --context "focus time"
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

## Agent Instructions

Use this tool to update session and pane statuses; update pane context when needed. Do not modify session context unless explicitly requested. The tool runs inside tmux, so always include tmux context (session name and pane ID) when updating metadata. Use `jkl --help` to review available commands.

`jkl upsert` details:

```
Usage: jkl upsert [OPTIONS] [SESSION_NAME]...

Arguments:
  [SESSION_NAME]...

Options:
      --session-id <SESSION_ID>
      --pane-id <PANE_ID>
      --status <STATUS>
      --context <CONTEXT>...
```

Examples:

- `jkl upsert <session_name...> [--session-id <session_id>] [--status <status>] [--context <text...>]` upserts session metadata.
- `jkl upsert <session_name...> --pane-id <pane_id> [--status <status>] [--context <text...>]` upserts pane metadata.

Sample commands:

```
# Update session status
jkl upsert "work" --status working

# Update pane status
jkl upsert "work" --pane-id %1 --status waiting

# Update pane context
jkl upsert "work" --pane-id %1 --context "debugging timeout"
```
