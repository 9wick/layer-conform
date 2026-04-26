# layer-conform

Detect "style deviations" within a layer of a TypeScript/JavaScript project — i.e. find functions that look different from the rest of their layer.

This MVP only supports a single-pair comparison via CLI flags.

## Build

```sh
cargo build --release
```

## Usage (MVP)

```sh
layer-conform check \
  --file src/repositories/useProduct.ts \
  --symbol useProduct \
  --golden src/repositories/useUser.ts:useUser
```

Output:

```
src/repositories/useProduct.ts:useProduct vs src/repositories/useUser.ts:useUser
  overall=0.412  shape=0.380  calls=0.000  imports=0.500  signature=1.000
  missing calls: ["useSWR"]
  extra calls:   ["fetch", "useEffect", "useState"]
```

## Status

- ✅ Phase 1a: lc-core (APTED + TSED + 4-axis similarity)
- ✅ Phase 1b: lc-ts (FunctionDeclaration only) + CLI 1-pair compare
- ⏳ Phase 2: config-driven, full extraction (Arrow/Method/etc.), `--explain` / `why`, `init`
- ⏳ Phase 3: baseline, `--changed`, `--summary`
- ⏳ Phase 4: `init --auto`, multi-language, distribution

See `docs/superpowers/specs/2026-04-26-layer-conform-design.md`.
