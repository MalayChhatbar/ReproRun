# ReproRun

ReproRun is a deterministic execution engine for running commands with controlled inputs and reproducible outputs.

It is designed for the class of problems where "works on my machine" is unacceptable: local development, CI, flaky test diagnosis, and debugging cache correctness.

## Current MVP

- strict YAML config loading with interpolation and validation
- deterministic execution environment (`LC_ALL=C`, `TZ=UTC`, seed/time injection)
- best-effort filesystem sandboxing with allowlist, denylist, and snapshot staging
- safe command execution without shell mode by default
- stdout/stderr capture, timeout handling, and exit reporting
- BLAKE3-based run hashing over command, config, environment, working directory, git metadata fields, and input files
- local content-addressed artifact cache in `.runs/<hash>/`
- reproducibility checking via repeated execution (`repro check`)
- human and JSON diff output for cached runs (`repro diff`)
- security hardening around cache boundaries, cache artifact integrity, and timed-out process cleanup
- workspace-wide regression tests with normal-case and edge-case coverage

## Quick Start

```powershell
cargo build --workspace
cargo run -p reprorun-cli -- init
cargo run -p reprorun-cli -- run repro.yaml --json
cargo run -p reprorun-cli -- check examples/flaky.yaml --json
```

## CLI

```text
repro init [path] [--force]
repro run [config] [--no-cache] [--json]
repro check [config] [--runs N] [--json]
repro diff <left-hash> <right-hash> [--json] [--no-color]
repro cache clean
repro cache prune --max-bytes <n>
```

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
|   `-- config/
|-- docs/
|-- examples/
`-- .github/
```

## Validation

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Documentation

- [docs/README.md](docs/README.md): full documentation index
- [docs/architecture.md](docs/architecture.md): crate responsibilities and execution flow
- [docs/configuration.md](docs/configuration.md): full `repro.yaml` reference
- [docs/cli.md](docs/cli.md): command reference and examples
- [docs/determinism.md](docs/determinism.md): what is controlled, hashed, and compared
- [docs/testing.md](docs/testing.md): current test strategy and coverage expectations
- [docs/security.md](docs/security.md): security model, hardening, and current limits
- [docs/development.md](docs/development.md): day-to-day contributor workflow
- [docs/release.md](docs/release.md): release workflow and versioning
- [docs/github.md](docs/github.md): repository bootstrap and GitHub setup

## License

MIT
