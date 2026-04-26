# Development

## Prerequisites

- Rust 1.75+

## Common Commands

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Local Smoke Tests

```powershell
cargo run -p reprorun-cli -- init
cargo run -p reprorun-cli -- run repro.yaml --json
cargo run -p reprorun-cli -- check examples/flaky.yaml --json
```

## Project Conventions

- Keep commits scoped to one logical change.
- Add tests for behavior changes.
- Keep output deterministic and machine-readable where practical.
