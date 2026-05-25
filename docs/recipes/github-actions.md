# Recipe: GitHub Actions

Minimal pattern:

```yaml
- name: Build ReproRun
  run: cargo build --release -p reprorun-cli

- name: Check reproducibility
  run: .\target\release\repro.exe check repro.yaml --runs 3 --json
  shell: pwsh
```

Notes:

- `repro check` exits `1` on nondeterminism, which is useful for CI gates
- `--json` makes the output easier to archive or parse in later steps
- commit `repro.yaml` so the CI machine uses the same declared inputs as local development
