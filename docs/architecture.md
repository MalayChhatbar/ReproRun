# Architecture

ReproRun is organized as a Rust workspace with focused crates:

- `reprorun-cli`: user-facing `repro` command.
- `reprorun-core`: orchestration layer for `run`, `check`, and `diff`.
- `reprorun-config`: strict YAML parser and validation.
- `reprorun-sandbox`: best-effort filesystem policy and snapshot staging.
- `reprorun-executor`: process execution, deterministic env injection, timeout, capture.
- `reprorun-hasher`: BLAKE3 hashing for deterministic run identity.
- `reprorun-cache`: content-addressed artifact store in `.runs/<hash>/`.
- `reprorun-reporter`: human/JSON diff generation.

## Execution Flow

1. Read and validate `repro.yaml`.
2. Resolve sandbox policy and prepare snapshot workspace.
3. Build deterministic execution environment (`LC_ALL=C`, `TZ=UTC`, seed/time vars).
4. Hash normalized command/config/input files/env into a run hash.
5. Return cached result if available (unless disabled).
6. Execute command and capture outputs.
7. Persist artifacts and metadata in `.runs/<hash>/`.

## Artifact Layout

Each run hash directory contains:

- `meta.json`
- `stdout.bin`
- `stderr.bin`
- `config.yaml`
- `env.json`
