# Configuration Reference

Default config filename: `repro.yaml`

The config parser is strict:

- unknown fields are rejected
- `${VAR}` interpolation happens before validation
- missing interpolated variables are an error
- invalid command definitions are rejected

## Full Schema

```yaml
command: ["echo", "hello"]
working_dir: .
stdin: "optional stdin"
env:
  DEBUG: "false"
filesystem:
  mode: sandbox
  allow: []
  deny: []
  snapshot_max_bytes: 104857600
limits:
  timeout_secs: 5
  cpu_time_secs: 2
  memory_mb: 512
  process_limit: 64
  fd_limit: 256
  output_max_bytes: 10485760
determinism:
  seed: 42
  time_epoch: 1700000000
network:
  enabled: false
check:
  runs: 3
```

## Top-Level Fields

### `command`

Supported forms:

```yaml
command: ["python", "app.py"]
```

or

```yaml
command: "python app.py"
```

Current behavior:

- argv form is the preferred mode
- shell-string form is accepted by the config parser
- shell execution is still disabled by the executor by default
- empty argv and blank shell strings are rejected

### `working_dir`

- optional
- defaults to the repository base directory passed to the run
- may include `${VAR}` interpolation
- is canonicalized before execution

### `stdin`

- optional string content injected into the command's stdin
- may include `${VAR}` interpolation

### `env`

- map of environment variables added to the deterministic execution environment
- values may include `${VAR}` interpolation
- ordering is normalized before hashing

Baseline environment always added by ReproRun:

- `LC_ALL=C`
- `TZ=UTC`
- `REPRORUN_SEED=<seed>` when available
- `REPRORUN_TIME_EPOCH=<epoch>` when available

### `filesystem`

Controls input-file policy and snapshot staging.

#### `mode`

Supported values:

- `read_only`
- `sandbox`
- `snapshot`

Current behavior:

- `sandbox` and `snapshot` both trigger snapshot staging of allowlisted paths
- `read_only` currently resolves allowlist paths but skips copy staging

#### `allow`

- list of files or directories treated as explicit inputs
- resolved relative to the repository base when not absolute
- must resolve inside the repository base directory
- are both snapshotted and hashed as deterministic inputs

#### `deny`

- list of blocked files or directories
- checked after path canonicalization

#### `snapshot_max_bytes`

- maximum total bytes copied into the snapshot area
- default: `104857600` (`100 MiB`)
- if exceeded, sandbox preparation fails before execution

### `limits`

#### `timeout_secs`

- optional execution timeout in seconds
- enforced in the executor
- on timeout, ReproRun attempts graceful wait and then kill
- Windows kill behavior is process-tree-aware

#### `cpu_time_secs`, `memory_mb`, `process_limit`, `fd_limit`

- represented in schema
- currently not fully enforced across all platforms
- retained so config shape is already future-compatible

#### `output_max_bytes`

- capture cap for both stdout and stderr
- output beyond the cap is truncated and marked in metadata
- default: `10485760` (`10 MiB`)

### `determinism`

#### `seed`

- optional explicit seed
- when omitted, current implementation derives a deterministic seed from config content

#### `time_epoch`

- optional explicit epoch value
- when omitted, current implementation uses `0`

### `network`

#### `enabled`

- currently schema-level only
- present for future control surface
- do not treat this as kernel-enforced network isolation in v1

### `check`

#### `runs`

- number of repeated executions for `repro check`
- default: `3`
- must be `>= 1`

## Defaults

If omitted, current defaults are:

- `filesystem.mode = sandbox`
- `filesystem.allow = []`
- `filesystem.deny = []`
- `filesystem.snapshot_max_bytes = 104857600`
- `limits.output_max_bytes = 10485760`
- `network.enabled = false`
- `check.runs = 3`

## Interpolation Rules

Interpolation syntax:

```yaml
env:
  TOKEN: "${HOST_TOKEN}"
```

Current behavior:

- `${NAME}` requires a closing `}`
- `${}` is invalid
- missing variables fail config load
- interpolation is applied to:
  - `command`
  - `stdin`
  - env values
  - `working_dir`
  - filesystem allow/deny paths

## Invalid Config Examples

Unknown field:

```yaml
command: ["echo", "ok"]
unknown_field: true
```

Invalid empty argv:

```yaml
command: []
```

Invalid check count:

```yaml
command: ["echo", "ok"]
check:
  runs: 0
```
