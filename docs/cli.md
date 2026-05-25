# CLI Reference

Binary name: `repro`

The CLI is intentionally non-interactive and CI-friendly. Successful commands write useful output to stdout. Configuration, runtime, and command-resolution failures exit with status `2`.

## Version Output

`repro --version` includes:

- crate version
- full git commit
- short git commit
- exact tag when available
- dirty working tree state at build time

## Global Flags

- `-q`, `--quiet`
  - suppresses streamed child-process output during `repro run`
- `-v`
  - parsed and reserved for future verbosity levels

## Command Reference

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

Execute one configuration.

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
- a note when the result came from cache rather than a fresh execution

JSON output includes:

- `hash`
- `from_cache`
- `exit_code`
- `exit_reason`
- `duration_ms`

Exit behavior:

- `0` when execution succeeds or a cached artifact is returned
- `2` on CLI/config/runtime failures

### `repro check [config] [--runs N] [--json]`

Execute the same config repeatedly and compare observed results.

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

- `0` when all repeated runs match
- `1` when nondeterminism is detected
- `2` on CLI/config/runtime failures

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

- `0` when no differences exist
- `1` when differences exist
- `2` on invalid input or cache-load failures

### `repro inspect <hash> [--json]`

Inspect one cached run artifact.

Examples:

```powershell
repro inspect <hash>
repro inspect <hash> --json
```

Human output includes:

- hash
- exit code
- exit reason
- duration
- stdout/stderr byte sizes
- truncation flags
- stored config snapshot
- stored environment snapshot

### `repro list [--limit N] [--json]`

List cached runs in most-recent-first order.

Examples:

```powershell
repro list
repro list --limit 5
repro list --json
```

Current summary fields:

- hash
- exit code
- exit reason
- duration
- stdout/stderr byte sizes
- cache entry modified time in Unix milliseconds

### `repro doctor [--json]`

Report repository-local health information.

Examples:

```powershell
repro doctor
repro doctor --json
```

Current checks:

- current working directory
- whether `repro.yaml` is present
- number of cached runs
- whether git metadata is available
- current git commit
- whether the tracked working tree is dirty
- warnings for missing config or missing git context

### `repro hash explain [config] [--json]`

Show the current hash inputs for one configuration.

Examples:

```powershell
repro hash explain
repro hash explain examples/repro.yaml
repro hash explain repro.yaml --json
```

Current explanation fields:

- final hash
- normalized command
- effective working directory
- effective seed
- effective time epoch
- normalized environment
- resolved input files
- git commit
- git dirty state

This command explains what is currently being hashed. It does not yet compute a before/after delta between two different configurations.

### `repro completions <shell>`

Generate shell completions.

Supported shells:

- `bash`
- `elvish`
- `fish`
- `powershell`
- `zsh`

Examples:

```powershell
repro completions powershell > repro.ps1
repro completions bash > repro.bash
```

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
