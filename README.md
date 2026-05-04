# layer-conform

[![crates.io](https://img.shields.io/crates/v/layer-conform.svg)](https://crates.io/crates/layer-conform)
[![docs.rs](https://img.shields.io/docsrs/layer-conform)](https://docs.rs/layer-conform)
[![license: MIT](https://img.shields.io/crates/l/layer-conform.svg)](./LICENSE)

[日本語版 README](./README-ja.md)

Detect "style deviations" within an architectural layer of your codebase — i.e. find functions that look different from the rest of their layer.

## Supported languages

- TypeScript / JavaScript
- Rust

## Install

```sh
cargo install --locked layer-conform
```

## Quickstart

Generate a starter config:

```sh
layer-conform init
```

Edit `.layer-conform.json` to point at your golden(s) and the glob of files that
should look like them.

For TypeScript:

```json
{
  "version": 1,
  "rules": [
    {
      "id": "repositories",
      "golden": "src/repositories/useUser.ts:useUser",
      "applyTo": "src/repositories/**/*.ts",
      "threshold": 0.7
    }
  ]
}
```

For Rust:

```json
{
  "version": 1,
  "rules": [
    {
      "id": "handlers",
      "golden": "src/handlers/get_user.rs:get_user",
      "applyTo": "src/handlers/**/*.rs",
      "threshold": 0.7
    }
  ]
}
```

Then run:

```sh
layer-conform        # exit 1 when deviations are found
```

Skip a single function with an inline directive (TypeScript only):

```ts
// layer-conform-ignore: legacy adapter, will be deleted in Q3
function useLegacy() { ... }
```

## Subcommands

| Command | What it does |
|---|---|
| `layer-conform` (no args) / `layer-conform check` | Run every rule against every matching file |
| `layer-conform check --explain <FILE>` | Same, but show only one file's detail |
| `layer-conform why <FILE>` | List every rule + golden that touches `<FILE>` and its scoring |
| `layer-conform init [--force]` | Write a starter `.layer-conform.json` |

## Global flags

| Flag | Effect |
|---|---|
| `--threshold <N>` | Override every rule's threshold (e.g. `0.5`) |
| `--no-color` | Disable ANSI colors |
| `--json` | Emit machine-readable JSON |

## Configuration

Each rule supports the following polymorphic shorthands:

- `golden`: `"file:symbol"` | `{ "file": "...", "symbol": "..." }` | array of either
- `applyTo` / `ignore`: `string` | `string[]`
- `threshold`: optional `number` (default `0.7`)
- `disabled`: optional `boolean`

Multi-golden picks the highest-scoring golden per function:

```json
{ "id": "data", "golden": ["a.ts:hookA", "b.ts:hookB"], "applyTo": "src/**/*.ts" }
```

## License

MIT
