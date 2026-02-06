---
name: jkl-cli
description: Operate the jkl/jkl2 CLI and TUI to inspect tmux sessions/panes and manage session or pane metadata stored in ~/.config/jkl/session_context.json. Use when an agent needs to launch the jkl TUI or pane-status selector, upsert session/pane status or context, rename a session entry by session id, or troubleshoot jkl metadata and tmux integration.
---

# JKL CLI

## Quick Start
- Confirm which binary is installed: prefer `jkl`; if missing, try `jkl2`.
- Run inside tmux for TUI features.
- Launch the TUI: `jkl tui`.
- Open pane status selector: `jkl tui --pane-state --session-name "<session_name...>" --pane-id "<pane_id>"`.

## Safety Notes
- Always include the tmux session name when updating metadata.
- When updating a pane, always include `--pane-id`.
- Update pane context when needed; do not modify session context unless explicitly requested.
- Avoid editing `~/.config/jkl/session_context.json` directly unless asked.

## Command Map
- `tui`
  - `jkl tui` opens the main TUI.
  - `jkl tui --pane-state --session-name <session_name...> --pane-id <pane_id>` opens the pane status selector for a specific pane.
- `upsert`
  - Session metadata: `jkl upsert <session_name...> [--session-id <session_id>] [--status <status>] [--context <text...>]`.
  - Pane metadata: `jkl upsert <session_name...> --pane-id <pane_id> [--status <status>] [--context <text...>]`.
- `rename`
  - `jkl rename <session_id> <session_name...>` renames the session entry keyed by `session_id`.

## Status Values
- Use `working`, `waiting`, `done`, or `none` (case-insensitive).
- Omit `--status` to leave the current status unchanged.

## TMUX Context Helpers
- Session name: `tmux display-message -p '#S'`
- Session id: `tmux display-message -p '#{session_id}'`
- Pane id: `tmux display-message -p '#{pane_id}'`

## Data Locations
- Context file: `~/.config/jkl/session_context.json` (created automatically if missing).
- Log file: `~/.config/jkl/jkl.log`.

## Examples
- Update session status:
  - `jkl upsert "work" --status working`
- Update pane status and context:
  - `jkl upsert "work" --pane-id %1 --status waiting --context "debugging timeout"`
- Rename a session entry:
  - `jkl rename "${SESSION_ID}" "new name"`

## Notes
- Multi-word session names and context values can be passed as multiple tokens; use `--` before options if a value could be parsed as a flag.
