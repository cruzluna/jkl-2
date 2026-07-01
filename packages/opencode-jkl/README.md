# opencode-jkl

TypeScript OpenCode plugin that syncs the current tmux pane into the [`jkl`](https://github.com/cruzluna/jkl-2) status view.

## Requirements

- `jkl` installed and available on `PATH`
- OpenCode running inside tmux

## Install

Use `jkl init`:

```bash
jkl init hooks --tool opencode --scope local --non-interactive
```

Or add the plugin to `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "plugin": ["opencode-jkl"]
}
```

OpenCode installs npm plugins automatically with Bun at startup.

## Behavior

- OpenCode `session.status` `busy` and `retry` events set the current tmux pane to `working`.
- OpenCode `session.status` `idle` and deprecated `session.idle` events set the current tmux pane to `waiting`.
- Outside tmux, or when `jkl` is unavailable, the plugin exits without changing anything.

## Publish

From this directory:

```bash
npm test
npm run typecheck
npm pack --dry-run
npm publish --access public
```
