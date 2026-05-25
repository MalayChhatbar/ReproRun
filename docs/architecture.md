# Architecture

ReproRun is a Rust workspace organized around explicit execution stages. The current implementation is intentionally modular even though the feature set is still MVP-level.

## Crates

- `reprorun-cli`
  - parses CLI input
  - loads config files
  - renders user-facing output
  - exposes `init`, `run`, `check`, `diff`, and cache commands
- `reprorun-core`
  - orchestrates the end-to-end run lifecycle
  - prepares sandbox state
  - computes run hashes
  - handles cache lookup and store
  - drives reproducibility checks and run diffs
- `reprorun-config`
  - defines the strict YAML schema
  - applies `${VAR}` interpolation
  - enforces validation rules and defaults
- `reprorun-sandbox`
  - canonicalizes allow/deny policy paths
  - enforces repository-base boundaries
  - snapshots allowlisted files into an isolated temp tree
- `reprorun-executor`
  - launches commands without shell mode by default
  - injects normalized environment variables
  - captures stdout/stderr
  - enforces timeout behavior
  - kills timed-out process trees on Windows
- `reprorun-hasher`
  - builds deterministic BLAKE3 run hashes
  - incorporates command/config/env/working-dir/input-file state
- `reprorun-cache`
  - persists content-addressed run artifacts under `.runs/<hash>/`
  - validates cache root boundaries
  - validates artifact metadata integrity
  - atomically promotes newly written artifacts
- `reprorun-reporter`
  - generates human-readable and JSON diff output

## High-Level Execution Flow

### `repro run`

1. CLI loads `repro.yaml` (or a user-provided path).
2. Config parsing applies defaults, interpolation, and validation.
3. Sandbox preparation resolves and validates filesystem policy.
4. Core collects canonical input files from the resolved allowlist.
5. Hasher computes a run hash from normalized run inputs.
6. Cache is consulted unless `--no-cache` is used.
7. Executor launches the command with deterministic environment settings.
8. Captured outputs and metadata are stored atomically in `.runs/<hash>/`.
9. CLI renders either text or JSON output.

### `repro check`

1. The config is parsed once.
2. The same config is executed repeatedly, with cache disabled.
3. Each run result is compared byte-for-byte for `stdout`, `stderr`, and exit code.
4. The first observed difference is retained for reporting.
5. The command exits non-zero if nondeterminism is detected.

### `repro diff`

1. Two cached run hashes are loaded from `.runs/`.
2. Cache paths are validated before read.
3. Reporter compares `stdout`, `stderr`, and exit code.
4. The command can emit either human diff text or JSON.

## Current Artifact Layout

Each cached run is stored under:

```text
.runs/<hash>/
|-- meta.json
|-- stdout.bin
|-- stderr.bin
|-- config.yaml
`-- env.json
```

### `meta.json`

Currently stores:

- `hash`
- `exit_code`
- `exit_reason`
- `duration_ms`
- `stdout_truncated`
- `stderr_truncated`

## Temporary Sandbox Layout

Snapshot staging currently lives under:

```text
tmp/sandbox/snapshot-<run-id>/
```

This is a best-effort implementation. It is used to stage allowed filesystem inputs, not to provide a hardened OS-level sandbox.

## Important Current Design Choices

- Determinism is primarily input normalization plus repeated-run comparison.
- Filesystem policy is repository-base-scoped: allowlisted paths must resolve under the base directory.
- Cache trust is limited by boundary checks and metadata validation, not cryptographic signing.
- Timeout handling on Windows is process-tree-aware; on non-Windows it is currently best-effort direct-child termination.

## Current Limits

- No syscall-level time freezing.
- No random-device interception.
- No full kernel-enforced sandbox.
- No remote cache or distributed execution.
- No plugin system or stable library API.
