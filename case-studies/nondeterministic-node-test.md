# Case Study: Nondeterministic Node Test

Problem:

- a Node test was reading an undeclared config file outside the expected source tree

ReproRun workflow:

```powershell
repro hash explain repro.yaml
```

Value:

- the resolved input-file list exposed that the current allowlist was incomplete
- the issue became a config problem rather than a vague CI mystery
