# Testing

ReproRun now has a workspace-wide regression suite intended to test behavior, not just compilation.

## Current Validation Commands

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Testing Philosophy

The current suite aims to cover:

- expected successful behavior
- input validation failures
- boundary conditions
- regression cases for previously fixed bugs and vulnerabilities
- deterministic vs nondeterministic execution behavior

Tests should prove the feature contract. They should not exist only to preserve the current implementation structure.

## Coverage by Crate

### `reprorun-config`

Current tests cover:

- minimal valid config with defaults
- unknown field rejection
- `${VAR}` interpolation success
- interpolation failure for missing variables
- malformed interpolation syntax
- empty argv rejection
- blank shell-command rejection
- non-zero `check.runs` enforcement
- path interpolation
- file-loading error propagation

### `reprorun-cache`

Current tests cover:

- store/load round trip
- presence checks
- clean behavior
- prune behavior
- invalid hash rejection
- metadata hash mismatch rejection
- corrupted artifact repair
- idempotent store behavior for valid existing artifacts
- prune behavior when cache root is missing

### `reprorun-executor`

Current tests cover:

- shell mode disabled by default
- empty argv rejection
- stdout capture
- stderr capture
- stdin injection
- seed/time env injection
- env propagation
- output truncation
- zero-byte capture limit
- timeout handling
- Windows timed-out process-tree cleanup

### `reprorun-hasher`

Current tests cover:

- hash stability for identical inputs
- hash changes for file content changes
- hash changes for env changes
- hash changes for git metadata changes
- canonical fast-path equality
- file order stability
- property-based env-order stability

### `reprorun-sandbox`

Current tests cover:

- allow-path resolution
- deny-path rejection
- outside-base rejection
- snapshot copy behavior
- read-only mode no-copy behavior
- snapshot size cap enforcement

### `reprorun-core`

Current tests cover:

- cache hit behavior on repeated stable run
- deterministic `check` success
- nondeterministic `check` failure
- outside-base allowlist rejection via orchestration
- run diffing by cached hashes
- config file load round trip

### `reprorun-reporter`

Current tests cover:

- no-diff behavior
- stdout diff rendering
- stderr diff rendering
- exit-code diff rendering
- JSON diff serialization
- ANSI color output when color is explicitly requested

### `reprorun-cli`

Current tests cover:

- `init` writes config
- `init` refuses overwrite
- `init --force` overwrites
- parent directory creation
- generated config validity
- embedded version metadata presence

## What the Current Tests Do Not Guarantee

Even with the current suite, tests do not fully prove:

- kernel-level sandboxing correctness
- full cross-platform process-tree semantics on every OS
- real network isolation
- correctness under extreme parallel write contention

Those remain future expansion areas.

## When Adding New Features

At minimum, add:

- one normal-case test
- one failure-case test
- one edge-case or regression test if the feature touches boundaries, cache, process execution, or filesystem behavior
