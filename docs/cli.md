# CLI Reference

Binary: `repro`

## Global Flags

- `-q`, `--quiet`
- `-v` (repeatable)

## Commands

## `repro init [path] [--force]`

Create a default config file (`repro.yaml` by default).

## `repro run [config] [--no-cache] [--json]`

Execute a config once.

Outputs hash, cache status, exit result.

## `repro check [config] [--runs N] [--json]`

Run multiple times and compare exact output bytes and exit code.

Returns non-zero when nondeterminism is detected.

## `repro diff <left-hash> <right-hash> [--json] [--no-color]`

Diff cached runs by hash.

Returns non-zero when any difference exists.

## `repro cache clean`

Remove `.runs/` cache.

## `repro cache prune --max-bytes <n>`

Prune oldest run artifacts until total size is within limit.
