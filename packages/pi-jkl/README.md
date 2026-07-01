# pi-jkl

TypeScript Pi extension that syncs the current tmux pane into the [`jkl`](https://github.com/cruzluna/jkl-2) status view.

## Requirements

- `jkl` installed and available on `PATH`
- Pi running inside tmux

## Install

Use `jkl init`:

```bash
jkl init hooks --tool pi --scope local --non-interactive
```

Or install directly with Pi:

```bash
pi install npm:pi-jkl
```

Project-local installs use:

```bash
pi install -l npm:pi-jkl
```

## Behavior

- Pi `agent_start` sets the current tmux pane to `working`.
- Pi `session_start`, `agent_end`, and `session_shutdown` set the current tmux pane to `waiting`.
- Outside tmux, or when `jkl` is unavailable, the extension exits without changing anything.

## Publish

From this directory:

```bash
npm test
npm run typecheck
npm pack --dry-run
npm publish --access public
```
