# Configuration Reference

Default file: `repro.yaml`

Unknown fields are rejected.

## Top-level Fields

```yaml
command: ["echo", "hello"]
working_dir: .
stdin: "optional input"
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

## `command`

- Preferred: argv array.
- Shell string is parsed but shell execution is disabled by default in v1.

## `env`

- Supports `${VAR}` interpolation from host environment at config load time.
- Environment is normalized for deterministic execution.

## `filesystem`

- `mode`: `read_only`, `sandbox`, or `snapshot`.
- `allow`: paths included for snapshot/hash input.
- `deny`: blocked paths.
- `snapshot_max_bytes`: cap for snapshot staging.

## `limits`

- `timeout_secs`: graceful wait then hard kill.
- `output_max_bytes`: cap for captured stdout/stderr bytes.
- Other limits are schema-level in v1 and can be platform-specific in enforcement.

## `determinism`

- `seed`: deterministic seed value for run env.
- `time_epoch`: deterministic epoch value for run env.

## `check`

- `runs`: number of repeat runs for reproducibility checks.
