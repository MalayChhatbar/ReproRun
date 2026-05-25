# Development

This document describes the current contributor workflow for ReproRun.

## Toolchain

- Rust `1.75+`
- Cargo
- Git

## Common Commands

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Local Workflow

Typical flow:

1. make a focused change
2. add or update tests with that change
3. run `cargo fmt --all`
4. run `cargo clippy --workspace --all-targets -- -D warnings`
5. run `cargo test --workspace`
6. commit only after the workspace is green

## Workspace Conventions

- keep crates focused on one responsibility
- prefer deterministic behavior over convenience defaults
- prefer strict validation over silent fallback
- treat config input as untrusted
- keep CLI behavior non-interactive and scriptable

## Current Testing Expectations

Behavior changes should usually include:

- normal-case coverage
- edge-case coverage
- regression coverage for any bug or vulnerability fixed

Examples:

- config parsing should cover both valid and invalid forms
- cache changes should cover corruption and invalid path handling
- executor changes should cover stdout, stderr, stdin, timeout, and env injection

## Useful Smoke Commands

```powershell
cargo run -p reprorun-cli -- init
cargo run -p reprorun-cli -- run repro.yaml --json
cargo run -p reprorun-cli -- run examples/repro.yaml --json
cargo run -p reprorun-cli -- check examples/flaky.yaml --json
```

## Notes on Platform Behavior

The current repository is developed and validated heavily on Windows.

Some behavior is intentionally best-effort on other platforms, especially around:

- process-tree termination
- OS-level resource isolation
- sandboxing depth
