# Case Study: Environment Mismatch

Problem:

- local and CI runs behaved differently because one machine supplied extra environment variables

ReproRun workflow:

```powershell
repro run repro.yaml
repro inspect <hash>
```

Value:

- normalized env handling reduced ambient leakage
- the stored environment snapshot showed the exact values used for the run
