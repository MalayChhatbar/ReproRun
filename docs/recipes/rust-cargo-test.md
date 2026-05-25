# Recipe: Rust `cargo test`

Use this when a Rust integration test is sensitive to local machine state.

```yaml
command: ["cargo", "test", "--test", "integration", "--", "--nocapture"]

env:
  RUST_BACKTRACE: "1"

filesystem:
  mode: sandbox
  allow:
    - Cargo.toml
    - Cargo.lock
    - src/
    - tests/

limits:
  timeout_secs: 60

determinism:
  seed: 42
  time_epoch: 1700000000
```

Suggested commands:

```powershell
repro run repro.yaml --no-cache
repro check repro.yaml --runs 3
```

Why `--no-cache` first:

- it confirms the command actually executes before you depend on cached artifacts
