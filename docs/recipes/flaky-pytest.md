# Recipe: Flaky Pytest Test

Use this when a Python test passes locally but flakes in CI.

`repro.yaml`

```yaml
command: ["pytest", "tests/test_flaky.py::test_case", "-q"]

env:
  PYTHONHASHSEED: "0"

filesystem:
  mode: sandbox
  allow:
    - tests/
    - src/
    - pyproject.toml

limits:
  timeout_secs: 30

determinism:
  seed: 42
  time_epoch: 1700000000
```

Commands:

```powershell
repro run repro.yaml
repro check repro.yaml --runs 5
repro hash explain repro.yaml
```

What to look for:

- if `repro check` fails, compare stdout/stderr for the first mismatch
- if the hash changes unexpectedly, run `repro hash explain` and inspect env/input files
- if you want the exact stored artifact, use `repro inspect <hash>`
