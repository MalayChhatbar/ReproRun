# Determinism Model

ReproRun v1 is not a "perfect replay VM". It is a deterministic execution framework built around normalized inputs, explicit hashing, and repeated-run verification.

## What ReproRun Controls

### Environment normalization

Every execution is launched with:

- a cleared environment baseline
- `LC_ALL=C`
- `TZ=UTC`
- optional `REPRORUN_SEED`
- optional `REPRORUN_TIME_EPOCH`

User-provided environment variables from config are then added on top.

### Filesystem input set

Deterministic file inputs come from the allowlist:

- allowlisted files are hashed directly
- allowlisted directories are walked recursively
- paths are canonicalized before acceptance
- paths must remain inside the repository base

### Working directory

- the effective working directory is canonicalized
- it is included in the run hash

### Config bytes

- the config file content is included in the run hash

### Git metadata fields

The hash model includes git fields in the hash input structure:

- `git_commit`
- `git_dirty`

Current note:

- the hash layer supports these fields directly
- whether and how they are populated depends on the current orchestration path

## What ReproRun Hashes

Current run hash inputs:

- normalized command representation
- normalized environment map
- canonical working directory
- raw config bytes
- seed
- time epoch
- OS
- architecture
- ReproRun version
- git metadata fields
- canonical input file paths
- input file contents

The current hashing algorithm is BLAKE3.

## What `repro check` Verifies

`repro check` does not compare hashes between runs. It compares execution outputs:

- stdout bytes
- stderr bytes
- exit code

If any of those differ across repeated runs, the execution is treated as nondeterministic.

## Current Best-Effort Areas

ReproRun v1 does not yet guarantee:

- syscall-level time freezing
- interception of OS random devices
- full kernel-level sandboxing
- strict low-level network isolation
- process-tree kill guarantees on every operating system

Current timeout behavior:

- Windows: explicit process-tree kill via `taskkill /T /F`
- non-Windows: best-effort direct child kill

## Practical Interpretation

Today, ReproRun is strongest when:

- your command behavior is mostly determined by config, env, working dir, and allowlisted files
- your command is launched locally
- you care about detecting nondeterminism, not proving formal replay equivalence

It is weaker when:

- the command consults hidden system state
- the command reads undeclared files outside the allowlist
- the command relies on external services or clocks not controlled by env configuration
