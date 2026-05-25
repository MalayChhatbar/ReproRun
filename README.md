# ReproRun

ReproRun is a deterministic execution engine for running commands with controlled inputs and reproducible outputs.

## Current Scope (MVP)

- deterministic execution controls (environment, seed, basic time epoch)
- strict YAML config (`repro.yaml`)
- best-effort filesystem snapshot sandbox (allowlist + denylist)
- command execution without shell by default
- output capture (`stdout`, `stderr`, exit code, timeout reason)
- content-addressed run hashing with BLAKE3
- local artifact cache in `.runs/<hash>/`
- reproducibility checks (`repro check`)
- run diffing in human and JSON formats (`repro diff`)

## Workspace Layout

```text
reprorun/
├── crates/
│   ├── cli/
│   ├── core/
│   ├── sandbox/
│   ├── executor/
│   ├── cache/
│   ├── hasher/
│   ├── config/
│   └── reporter/
```

## Quick Start

```powershell
cargo build --workspace
cargo run -p reprorun-cli -- run examples/repro.yaml
cargo run -p reprorun-cli -- check examples/flaky.yaml
```

## CLI

```text
repro init [path] [--force]
repro run [config]
repro check [config]
repro diff <hash1> <hash2>
repro cache clean
repro cache prune --max-bytes <n>
```

## Development

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Documentation

See [docs/README.md](docs/README.md) for full project documentation.

## License

MIT
