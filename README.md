# layer-conform

Detect "style deviations" within a layer of a TypeScript/JavaScript project — i.e. find functions that look different from the rest of their layer.

## Build

```sh
cargo build --release
```

## Quickstart

Generate a starter config:

```sh
layer-conform init
```

Edit `.layer-conform.json` to point at your golden(s) and the glob of files that
should look like them:

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

Then check the workspace:

```sh
layer-conform        # exit 1 when deviations are found
```

Skip a single function with an inline directive:

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
| `--json` | Emit machine-readable JSON (see spec §6) |

## Configuration shape

Each rule supports the following polymorphic shorthands:

- `golden`: `"file.ts:symbol"` | `{ "file": "...", "symbol": "..." }` | array of either
- `applyTo` / `ignore`: `string` | `string[]`
- `threshold`: optional `number` (default `0.7`)
- `disabled`: optional `boolean`

Multi-golden picks the highest-scoring golden per function:

```json
{ "id": "data", "golden": ["a.ts:hookA", "b.ts:hookB"], "applyTo": "src/**/*.ts" }
```

## Function kinds detected

`function fn() {}`, `const x = () => {}`, `const x = function() {}`,
`{ method() {} }`, `class C { method() {} prop = () => {} }`,
`export default function() {}`, `export default () => {}`.

## Status

- ✅ Phase 1a: lc-core (APTED + TSED + 4-axis similarity)
- ✅ Phase 1b: lc-ts (FunctionDeclaration only) + CLI 1-pair compare
- ✅ Phase 2: config-driven check, all 6 function kinds, multi-golden,
  `layer-conform-ignore` directive, `--explain` / `why`, `init`,
  `--threshold` / `--no-color` / `--json`
- ⏳ Phase 3: baseline, `--changed`, `--summary`
- ⏳ Phase 4: `init --auto`, multi-language, distribution

See `docs/superpowers/specs/2026-04-26-layer-conform-design.md`.
