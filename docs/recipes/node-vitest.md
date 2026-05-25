# Recipe: Node + Vitest

Use this when a Node test depends on ambient environment or undeclared files.

```yaml
command: ["pnpm", "vitest", "run", "src/example.test.ts"]

env:
  NODE_ENV: "test"

filesystem:
  mode: sandbox
  allow:
    - src/
    - package.json
    - pnpm-lock.yaml
    - vitest.config.ts

limits:
  timeout_secs: 20

determinism:
  seed: 42
  time_epoch: 1700000000
```

Suggested flow:

```powershell
repro run repro.yaml
repro run repro.yaml
repro list --limit 3
```

If the second run is not a cache hit, inspect the hash explanation and the allowed file set.
