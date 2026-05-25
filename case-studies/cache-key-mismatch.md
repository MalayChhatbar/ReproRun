# Case Study: Cache Key Mismatch

Problem:

- a command that should have reused a previous result kept missing cache

ReproRun workflow:

```powershell
repro list --limit 2
repro inspect <hash-a>
repro inspect <hash-b>
repro diff <hash-a> <hash-b>
```

Value:

- the cached snapshots made it obvious which run output changed
- the config and env snapshots narrowed the cause quickly
