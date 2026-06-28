<h1 align="center">jkl</h1>

<p align="center"><strong>Inspect your agent statuses in tmux sessions.</strong></p>

<p align="center">
  <img src="docs/assets/preview.png" alt="jkl TUI preview" width="960" />
</p>

<p align="center">The better agent list view</p>

## Requirements

- `tmux`

Quick links: [Install](#install) · [Getting started](#getting-started) · [Update](#update) · [Uninstall](#uninstall) · [Usage](#usage) · [Tmux plugin (TPM)](#tmux-plugin-tpm) · [Integrations](#integrations) · [Session context](#session-context) · [Development](#development)

## Install

### Binary (recommended)

```
curl -fsSL https://raw.githubusercontent.com/cruzluna/jkl-2/master/install.sh | bash
```

To test the installer without downloading or writing files:

```bash
curl -fsSL https://raw.githubusercontent.com/cruzluna/jkl-2/master/install.sh | bash -s -- --dry-run
```

Optional: set `JKL_INSTALL_DIR` to choose where binaries are installed.
On Linux, the installer picks:

- `*-unknown-linux-musl` on Alpine
- `x86_64-unknown-linux-gnu-al2` on Amazon Linux 2
- `*-unknown-linux-gnu` otherwise

The installer adds:

- `jkl`

After install, the script prints copy/paste prompts for Claude Code hooks, Kiro CLI hooks, and the `~/.tmux.conf` lines needed for the `jkl-2` TPM plugin. Reprint them later with `jkl init prompts`.

### Release Asset (manual)

Use install overrides instead of downloading/extracting assets manually.

```bash
# specific release tag:
JKL_INSTALL_TAG=v0.2.0 curl -fsSL https://raw.githubusercontent.com/cruzluna/jkl-2/master/install.sh | bash

# specific target (example: musl):
JKL_INSTALL_TARGET=x86_64-unknown-linux-musl curl -fsSL https://raw.githubusercontent.com/cruzluna/jkl-2/master/install.sh | bash

# both:
JKL_INSTALL_TAG=v0.2.0-rc.1 JKL_INSTALL_TARGET=aarch64-unknown-linux-gnu curl -fsSL https://raw.githubusercontent.com/cruzluna/jkl-2/master/install.sh | bash
```

Optional override:

- `JKL_INSTALL_ASSET_SUFFIX=-al2` to force an asset suffix (normally auto-detected on Amazon Linux 2)

### Cargo

```
cargo install --git https://github.com/cruzluna/jkl-2
```

If needed, ensure `~/.cargo/bin` is on your `PATH`.

## Getting started

1. **Configure with a coding agent**

> 💡 paste something like this into your agent session:

   ```bash
   claude "Help me set up my tmux.conf and Claude Code hooks with the jkl CLI/TUI. Run jkl init prompts"
   ```

    See [Integrations](#integrations) for more.


2. Open in tmux using **`<prefix> f`**.

See [Usage](#usage) for TUI shortcuts while the popup is open.

## Update

```
jkl update
jkl init fig-autocomplete
```

To include pre-releases:

```
jkl update --pre-release
jkl init fig-autocomplete
```

To include dev preview builds from the `dev` branch:

```
jkl update --pre-release --dev
jkl init fig-autocomplete
```

Master pre-releases are selected from tags that include `-rc.` (for example, `v0.2.0-rc.1`).
Dev previews are selected from tags that include `-dev.` (for example, `v0.2.0-dev.42.abc1234`).
`jkl update` keeps the installed binary target (for example, a `*-unknown-linux-musl` install updates to `*-unknown-linux-musl` assets). On Amazon Linux 2, `jkl update` automatically selects `-al2` release assets.

If you installed via Cargo:

```
cargo install --git https://github.com/cruzluna/jkl-2 --force
```

Then refresh Fig autocomplete:

```
jkl init fig-autocomplete
```

`jkl update` also prints the same Claude Code/Kiro hook prompts and the `~/.tmux.conf` prompt after the update finishes. Use `jkl init prompts` to print them again on demand.

## Uninstall

Uninstall the `jkl` binary from its current install location:

```
jkl uninstall
```

Also remove local metadata/logs in `~/.config/jkl`:

```
jkl uninstall --purge-data
```

<details>
<summary><strong>Release asset filenames</strong> (for maintainers — expected GitHub archive names)</summary>

The update command and install script expect GitHub release assets named:

- `jkl-x86_64-apple-darwin.tar.gz`
- `jkl-aarch64-apple-darwin.tar.gz`
- `jkl-x86_64-unknown-linux-gnu.tar.gz`
- `jkl-x86_64-unknown-linux-gnu-al2.tar.gz`
- `jkl-aarch64-unknown-linux-gnu.tar.gz`
- `jkl-x86_64-unknown-linux-musl.tar.gz`
- `jkl-aarch64-unknown-linux-musl.tar.gz`

Each archive should contain the `jkl` binary at the top level.

</details>

## Usage

### TUI (`jkl tui`)

Runs **inside tmux**. Session rows are ordered by **most recently used tmux session first**.

| Action | Keys |
| --- | --- |
| Quit | `q`, `Esc`, or `Ctrl+C` (`Ctrl+C` clears search/help first) |
| Keybinding help | `?`; leave help with `q`, `Q`, `Esc`, or `Ctrl+C` |
| Move selection | `↑` / `↓` or `j` / `k` |
| Expand / collapse selected session | `l` / `h` |
| Expand / collapse all sessions | `L` |
| Delete selected session, window, or pane | `x`, then **`x`** to confirm (anything else cancels) |
| Refresh pane list | `r` |
| Search panes/sessions/windows | `/`; `Esc` exits search |
| Switch to highlighted session | `Enter` |

**Pane status picker:** `jkl tui --pane-state --session-name <session_name…> --pane-id <pane_id>`.

Coding agents invoking `jkl` should assume a **tmux** environment and honor session/pane identifiers from tmux.

Multi-word session names or context arguments often need no quoting; use `--` to end positional tokens if parsing is ambiguous.

## Tmux plugin (TPM)

Add the plugin and reload TPM:

```
set -g @plugin 'cruzluna/jkl-2'

# Initialize TMUX plugin manager (keep at bottom)
run '~/.tmux/plugins/tpm/tpm'
```

If tpm fails to download the plugin:

```bash
tmux run-shell "~/.tmux/plugins/tpm/bin/install_plugins"
```

Default prefix bindings (only set when the key is currently unbound):

- `f`: open the agent view popup (`jkl tui`)
- `W`: prompt for context and run `jkl upsert '#S' --session-id '#{session_id}' --context <input>`
- `e`: open `~/.config/jkl/session_context.json` in `nvim`
- `S`: open pane status selector popup

You can configure or disable each key:

```tmux
# Set custom keys
set -g @jkl_key_agent_view 'J'
set -g @jkl_key_context 'C'
set -g @jkl_key_edit 'E'
set -g @jkl_key_pane_state 'P'

# Disable a binding
set -g @jkl_key_edit 'none'
```

You can configure the agent view popup size. Width and height accept tmux popup dimensions, including integer cells or percentages. Defaults are `40%` by `40%`.

```tmux
set -g @jkl_agent_view_popup_width '80'
set -g @jkl_agent_view_popup_height '20'
```

By default the plugin does not override existing tmux/user bindings. Recommended for a fresh setup: force the jkl defaults to bind even if tmux already has a key there:

```tmux
set -g @jkl_force_bind_keys 'on'
```

## Integrations

### Claude Code hooks

Claude Code hooks can be configured in `~/.claude/settings.json` (user), `.claude/settings.json` (project), or `.claude/settings.local.json` (local project). The example below marks the current tmux pane as `working` when you submit a prompt, `blocked` while Claude waits on user input or permission, and `idle` when Claude stops.

Initialize automatically:

```
jkl init hooks --tool claude --scope global --non-interactive
```

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working"
          }
        ]
      }
    ],
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status blocked"
          }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "permission_prompt|idle_prompt|elicitation_dialog",
        "hooks": [
          {
            "type": "command",
            "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status blocked"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status idle"
          }
        ]
      }
    ]
  }
}
```

Docs:

- https://code.claude.com/docs/en/hooks-guide
- https://code.claude.com/docs/en/hooks

### Kiro CLI hooks

Kiro CLI hooks are defined in an agent config file (for example `.kiro/agents/jkl.json` in a project, or `~/.kiro/agents/jkl.json` globally). The same pattern can be applied with `userPromptSubmit`, `preToolUse`, `postToolUse`, and `stop` hooks:

Initialize automatically:

```
jkl init hooks --tool kiro --scope local --non-interactive
```

To target specific Kiro agent config files:

```
jkl init hooks --tool kiro --scope local --non-interactive --agent-config .kiro/agents/dev.json .kiro/agents/reviewer.json
```

To use a different Kiro agents directory:

```
jkl init hooks --tool kiro --scope local --non-interactive --agent-config-dir ./custom/kiro/agents
```

Kiro hooks targeting behavior:

- `--agent-config` updates exactly the listed files (best for automation).
- `--agent-config-dir` chooses which directory the interactive selector scans.
- `--agent-config-dir` alone does not update every file automatically.
- In non-interactive mode, if `--agent-config` is omitted, jkl targets `<agent-config-dir>/jkl.json`.

```json
{
  "name": "jkl",
  "description": "Sync jkl status with Kiro activity",
  "hooks": {
    "userPromptSubmit": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working"
      }
    ],
    "preToolUse": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status blocked"
      }
    ],
    "postToolUse": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working"
      }
    ],
    "stop": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status idle"
      }
    ]
  }
}
```

Kiro also supports `preToolUse` and `postToolUse` hooks with tool `matcher` patterns (for example `execute_bash` or `fs_write`) if you want finer-grained automation.

Docs:

- https://kiro.dev/docs/cli/hooks/
- https://kiro.dev/docs/cli/custom-agents/configuration-reference/

### Cursor CLI hooks

Cursor hooks are configured in `hooks.json` at either the user level (`~/.cursor/hooks.json`) or the project level (`<project>/.cursor/hooks.json`).

The example below uses Agent hooks to mirror the Kiro behavior: set the current tmux pane to `working` on `beforeSubmitPrompt`, `blocked` before shell or MCP actions that may need approval, then back to `idle` on `stop`.

Initialize automatically:

```
jkl init hooks --tool cursor --scope local --non-interactive
```

To target specific Cursor hook config files:

```
jkl init hooks --tool cursor --scope local --non-interactive --agent-config .cursor/hooks.json ./custom/cursor/hooks.json
```

Cursor hooks targeting behavior:

- `--agent-config` updates exactly the listed files.
- In non-interactive mode, if `--agent-config` is omitted, jkl targets the default path from `--scope` (`<project>/.cursor/hooks.json` or `~/.cursor/hooks.json`).

```json
{
  "version": 1,
  "hooks": {
    "beforeSubmitPrompt": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working"
      }
    ],
    "beforeShellExecution": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status blocked"
      }
    ],
    "afterShellExecution": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working"
      }
    ],
    "beforeMCPExecution": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status blocked"
      }
    ],
    "afterMCPExecution": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status working"
      }
    ],
    "stop": [
      {
        "command": "[ -n \"$TMUX\" ] || exit 0; jkl upsert \"$(tmux display-message -p '#S')\" --session-id \"$(tmux display-message -p '#{session_id}')\" --pane-id \"$(tmux display-message -p '#{pane_id}')\" --status idle"
      }
    ]
  }
}
```

If you prefer scripts instead of inline commands, note the path difference from Cursor docs: user hooks run from `~/.cursor/` (for example `./hooks/script.sh`), while project hooks run from the project root (for example `.cursor/hooks/script.sh`).

Docs:

- https://cursor.com/docs/agent/hooks

### Copy/paste prompts

All `jkl init prompts` options and examples are under [Getting started](#getting-started).

### Fig autocomplete

A standalone Fig spec for `jkl` lives at:

- `completions/fig/jkl.ts`

Refresh autocomplete with:

```
jkl init fig-autocomplete
```

## Session context

The TUI reads optional metadata from `~/.config/jkl/session_context.json`. If the file does not exist, it is created with `{}` the first time you run the TUI.

Shape (keyed by `blake3(session_name)`):

```json
{
  "2f0d7b3b5e3b9b1d4b4b5b8b8e2e2e9a2d2d4d5f2f0f5e5f2d9b3f1a5c8e": {
    "session_name": "work",
    "session_status": "idle",
    "session_context": "my project",
    "windows": {
      "@10": {
        "window_id": "@10",
        "window_name": "editor"
      }
    },
    "panes": {
      "%1": {
        "window_id": "@10",
        "window_name": "editor",
        "pane_status": "working",
        "pane_context": "focus time"
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

Cleanup/repair example:

```
jkl sync
```

`jkl sync` keeps only sessions/windows/panes that still exist in tmux. Session matching is ID-first; if no ID match is found it falls back to session name. When a session is matched by ID but the name changed, jkl updates `session_name` and re-keys the JSON entry to `blake3(new_session_name)`.

Status values:

- `idle` (yellow)
- `working` (blue)
- `blocked` (red)
- `unknown` (gray)
- missing values render as `-`

## Testing

- `cargo check`
- `cargo test`

## Development

- Run TUI locally: `cargo run -- tui`
- Point tmux at a test server: `tmux -L test list-sessions`
- Use a temp context file: `HOME=/tmp/jkl-dev cargo run -- tui`

Contributor commands (fmt, Clippy, `cargo test --locked`): see [`AGENTS.md`](AGENTS.md).
