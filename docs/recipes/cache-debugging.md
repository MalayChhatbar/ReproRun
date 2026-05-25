# Recipe: Cache Debugging

Use this when a run should have been a cache hit but was not, or when a cache hit looks suspicious.

Suggested flow:

```powershell
repro run repro.yaml
repro run repro.yaml
repro list --limit 5
repro inspect <hash>
repro hash explain repro.yaml
```

Questions to answer:

- did the second run come from cache?
- does the stored config snapshot match what you expected?
- does the stored environment snapshot include surprising values?
- did the resolved input file set include files you forgot were part of the run?

If you have two suspicious runs:

```powershell
repro diff <hash-a> <hash-b>
```
