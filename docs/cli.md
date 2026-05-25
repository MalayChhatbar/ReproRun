# CLI Reference

Binary name: `repro`

The CLI is intentionally non-interactive and CI-friendly.

## Version Output

`repro --version` includes build metadata:

- crate version
- full git commit
- short git commit
- exact tag when available
- dirty working tree state at build time

## Global Flags

- `-q`, `--quiet`
  - suppresses streamed command output during `run`
- `-v`
  - currently parsed and reserved for future verbosity control

## Commands

### `repro init [path] [--force]`

Create a starter config file.

Examples:

```powershell
repro init
repro init .\configs\repro.yaml
repro init .\repro.yaml --force
```

Behavior:

- default output path is `repro.yaml`
- parent directories are created automatically
- existing files are not overwritten unless `--force` is provided

### `repro run [config] [--no-cache] [--json]`

Execute a configuration once.

Examples:

```powershell
repro run
repro run examples/repro.yaml
repro run examples/repro.yaml --json
repro run examples/repro.yaml --no-cache
```

Text output includes:

- `hash`
- `from_cache`
- `exit_code`
- `exit_reason`

JSON output includes:

- `hash`
- `from_cache`
- `exit_code`
- `exit_reason`
- `duration_ms`

Exit behavior:

- returns `0` when the command ran or a cached artifact was returned successfully
- returns `2` on CLI/config/runtime errors

### `repro check [config] [--runs N] [--json]`

Execute the same config repeatedly and compare results.

Examples:

```powershell
repro check
repro check examples/flaky.yaml
repro check examples/flaky.yaml --runs 5 --json
```

Comparison fields:

- stdout bytes
- stderr bytes
- exit code

Exit behavior:

- returns `0` when all repeated runs match
- returns `1` when nondeterminism is detected
- returns `2` on CLI/config/runtime errors

### `repro diff <left-hash> <right-hash> [--json] [--no-color]`

Compare two cached runs.

Examples:

```powershell
repro diff <hash-a> <hash-b>
repro diff <hash-a> <hash-b> --json
repro diff <hash-a> <hash-b> --no-color
```

Current diff scope:

- exit code
- stdout
- stderr

Exit behavior:

- returns `0` when no differences exist
- returns `1` when differences exist
- returns `2` on invalid input or load errors

### `repro cache clean`

Remove the cache root `.runs/`.

Behavior:

- validates that the cache root resolves inside the repository base before deleting

### `repro cache prune --max-bytes <n>`

Prune old cached runs until total cache size is at or below the requested limit.

Behavior:

- uses file size totals from cached artifact directories
- removes oldest entries first
- validates cache root boundaries before pruning

## Default Generated Config

`repro init` currently generates a config with:

- an `echo` command
- deterministic seed and time epoch
- `sandbox` filesystem mode
- `3` check runs
- `10 MiB` output cap
