# Fig autocomplete for `jkl`

This repository includes a Fig completion spec at:

- `completions/fig/jkl.ts`

## Why this approach

`jkl` currently uses `clap` v4, while the documented Rust `clap_complete_fig`
integration path is based on older `clap` v3-era crates.

Maintaining a standalone Fig spec keeps completions independent from the Rust
parser internals and avoids version-coupling.

## Upstreaming to Fig/Amazon Q

To publish completions broadly, contribute this spec to
[withfig/autocomplete](https://github.com/withfig/autocomplete):

1. Fork and clone `withfig/autocomplete`.
2. Copy `completions/fig/jkl.ts` into `src/jkl.ts` in that repo.
3. Follow their contribution/test process and open a PR.

Reference docs:

- [Fig integration guide](https://fig.io/docs/guides/integrating/integrations/getting-started)
- [withfig/autocomplete](https://github.com/withfig/autocomplete)
