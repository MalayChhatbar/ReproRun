# Determinism Model

ReproRun v1 targets practical determinism with controlled inputs.

## Controlled Inputs

- Normalized environment (`LC_ALL=C`, `TZ=UTC`).
- Seed/time injection via environment variables.
- Explicit command and working directory.
- Snapshot/hash of allowlisted input files.

## Hash Identity

Run hash includes:

- normalized command argv
- normalized environment map
- canonical working directory
- config bytes
- seed/time values
- OS/arch/tool version metadata
- git metadata
- content of input files

## Reproducibility Check

`repro check` executes N runs and compares:

- `stdout` bytes
- `stderr` bytes
- exit code

Any mismatch marks the run as nondeterministic.

## Current Limits

- No syscall-level time freezing in v1.
- No OS-level random-device interception in v1.
- Network is modeled in config but strict low-level blocking is best-effort in v1.
