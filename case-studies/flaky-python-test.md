# Case Study: Flaky Python Test

Problem:

- one pytest target sometimes passed locally and sometimes failed in CI

ReproRun workflow:

```powershell
repro check repro.yaml --runs 5
```

Value:

- repeated runs made the flake visible immediately
- stored run hashes made it easy to inspect the exact failed artifacts
