# ReproRun

ReproRun is a deterministic command runner for reproducing flaky tests, cache bugs, and CI/local environment mismatches.

It is built for the class of failures where "works on my machine" is not good enough:

- flaky tests
- hidden environment dependencies
- unstable cache keys
- local versus CI drift

## Demo

`repro.yaml`

```yaml
command: ["python", "app.py"]

env:
  DEBUG: "false"

filesystem:
  mode: sandbox
  allow:
    - src/

limits:
  timeout_secs: 5

determinism:
  seed: 42
  time_epoch: 1700000000
```

Run once:

```powershell
repro run repro.yaml
hash: 9b3d...
from_cache: false
exit_code: Some(0)
exit_reason: exited
```

Run again with the same inputs:

```powershell
repro run repro.yaml
hash: 9b3d...
from_cache: true
exit_code: Some(0)
exit_reason: exited
note: cached artifact returned, command was not executed
```

Check for nondeterminism:

```powershell
repro check examples/flaky.yaml --runs 3
deterministic: false
run[0] hash=...
run[1] hash=...
run[2] hash=...
## Stdout
--- run-a
+++ run-b
-first
+second
```

Explain a hash:

```powershell
repro hash explain repro.yaml
hash: 9b3d...
working_dir: D:\repo
seed: 42
time_epoch: 1700000000
git_commit: 0123abcd...
git_dirty: false
command:
- python
- app.py
env:
- DEBUG=false
- LC_ALL=C
- TZ=UTC
input_files:
- D:\repo\src\app.py
```

## Current MVP

- strict YAML config loading with `${VAR}` interpolation and unknown-field rejection
- deterministic execution environment with `LC_ALL=C`, `TZ=UTC`, seed injection, and fixed epoch injection
- best-effort filesystem sandboxing with allowlist, denylist, and snapshot staging
- safe command execution without shell mode by default
- stdout/stderr capture, timeout handling, truncation limits, and exit reporting
- BLAKE3-based run hashing over command, config, environment, working directory, git metadata, and declared input files
- local content-addressed artifact cache in `.runs/<hash>/`
- reproducibility checking via repeated execution
- human and JSON diff output for cached runs
- cache inspection, cache listing, and hash explanation commands
- shell completions for Bash, Zsh, Fish, PowerShell, and Elvish

## Install

Current install options:

```powershell
cargo install --path crates/cli
```

Or build locally:

```powershell
cargo build --release -p reprorun-cli
```

GitHub release binaries are produced by the release workflow on version tags. Once the first public release is published, add the exact release URL here.

## Quick Start

```powershell
cargo build --workspace
cargo run -p reprorun-cli -- init
cargo run -p reprorun-cli -- run repro.yaml --json
cargo run -p reprorun-cli -- check examples/flaky.yaml --json
cargo run -p reprorun-cli -- doctor
```

## CLI

```text
repro init [path] [--force]
repro run [config] [--no-cache] [--json]
repro check [config] [--runs N] [--json]
repro diff <left-hash> <right-hash> [--json] [--no-color]
repro inspect <hash> [--json]
repro list [--limit N] [--json]
repro doctor [--json]
repro hash explain [config] [--json]
repro completions <shell>
repro cache clean
repro cache prune --max-bytes <n>
```

## Important Limitation

ReproRun is not a security sandbox.

It is a reproducibility-oriented execution wrapper with filesystem policy enforcement and deterministic input control. It does not currently provide kernel-enforced isolation such as containers, seccomp, namespaces, AppArmor, SELinux, or macOS sandbox profiles.

Do not use ReproRun to execute untrusted code.

## Validation

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Documentation

- [docs/README.md](docs/README.md): documentation index
- [docs/architecture.md](docs/architecture.md): crate boundaries and execution flow
- [docs/configuration.md](docs/configuration.md): full `repro.yaml` reference
- [docs/cli.md](docs/cli.md): command reference and exit behavior
- [docs/determinism.md](docs/determinism.md): what is controlled, hashed, and compared
- [docs/security.md](docs/security.md): security model, hardening, and limits
- [docs/testing.md](docs/testing.md): validation strategy and regression suites
- [docs/recipes/README.md](docs/recipes/README.md): copy-paste usage recipes
- [case-studies/README.md](case-studies/README.md): practical examples and debugging narratives

## Workspace Layout

```text
reprorun/
|-- crates/
|   |-- cli/
|   |-- core/
|   |-- sandbox/
|   |-- executor/
|   |-- cache/
|   |-- hasher/
|   |-- config/
|   `-- reporter/
|-- docs/
|-- case-studies/
|-- examples/
`-- .github/
```

## License

MIT
