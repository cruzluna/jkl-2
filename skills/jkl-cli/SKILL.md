---
name: jkl-cli
description: Operate the jkl/jkl2 CLI and TUI to inspect tmux sessions/panes and manage session or pane metadata stored in ~/.config/jkl/session_context.json. Use when an agent needs to launch the jkl TUI or pane-status selector, upsert session/pane status or context, rename a session entry by session id, sync stale metadata against tmux, or troubleshoot jkl metadata and tmux integration.
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
- Set `--pane-name` only when initially labeling panes (usually at the beginning of a session); avoid repeatedly changing pane names during normal status/context updates.
- Update pane context when needed; do not modify session context unless explicitly requested.
- Avoid editing `~/.config/jkl/session_context.json` directly unless asked.

## Command Map
- `tui`
  - `jkl tui` opens the main TUI.
  - `jkl tui --pane-state --session-name <session_name...> --pane-id <pane_id>` opens the pane status selector for a specific pane.
- `upsert`
  - Session metadata: `jkl upsert <session_name...> [--session-id <session_id>] [--status <status>] [--context <text...>]`.
  - Pane metadata: `jkl upsert <session_name...> --pane-id <pane_id> [--pane-name <pane_name>] [--status <status>] [--context <text...>]`.
  - Prefer setting `--pane-name` once at session start, then using later upserts for status/context only.
- `rename`
  - `jkl rename <session_id> <session_name...>` renames the session entry keyed by `session_id`.
- `sync`
  - `jkl sync` removes persisted sessions/panes that no longer exist in tmux.
  - Session matching is ID-first and falls back to session name.
  - If a session was renamed in tmux, sync updates `session_name` and moves the entry to the new `blake3(session_name)` key.

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
- Set pane name at session start:
  - `jkl upsert "work" --pane-id %1 --pane-name "planner"`
- Rename a session entry:
  - `jkl rename "${SESSION_ID}" "new name"`
- Reconcile persisted metadata with tmux:
  - `jkl sync`

## Notes
- Multi-word session names and context values can be passed as multiple tokens; use `--` before options if a value could be parsed as a flag.
