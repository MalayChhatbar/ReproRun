# Security

This document describes the current security model of ReproRun and the hardening that exists today.

## Threat Model for v1

ReproRun v1 assumes:

- config input is untrusted and must be validated strictly
- cache contents may be stale or corrupted and must not be trusted blindly
- filesystem allowlists must not escape the repository base
- timed-out commands should not continue running silently when avoidable

ReproRun v1 does not assume:

- hostile kernel-level isolation
- perfect sandboxing against malicious native code
- full protection against every host-side side channel

## Current Hardening

### Config hardening

- strict YAML schema with unknown-field rejection
- interpolation errors fail config loading
- empty commands are rejected

### Filesystem boundary enforcement

- allowlist and denylist paths are canonicalized
- allowlisted paths must resolve inside the repository base
- snapshot copy paths are derived relative to the canonical base
- outside-base paths are rejected

### Cache hardening

- run hashes are validated as 64-char hex strings
- cache root resolution is validated against the repository base
- run artifact directories are validated against the cache root
- cache writes are atomic through temp-directory promotion
- corrupted existing artifacts are repaired on store
- metadata hash mismatches are rejected on load

### Executor hardening

- shell mode is disabled by default
- environment is explicitly cleared and rebuilt
- deterministic baseline env vars are always injected
- timeout handling on Windows kills the process tree via `taskkill /T /F`
- timeout handling on Unix creates a fresh session and kills the process group with `SIGKILL`

### Reporting

- `repro check` compares concrete observable outputs rather than trusting hashes alone

## Security-Relevant Implementation Limits

### Sandbox depth

Current sandboxing is filesystem-policy-oriented, not kernel-enforced.

That means:

- commands can still access host capabilities outside of what the current OS and executor behavior prevent
- network isolation is not a hardened boundary in v1

### Hidden host dependencies

Commands can still depend on undeclared host state if they access it directly outside the declared allowlist model.

### Cache authenticity

Current cache hardening protects against boundary escape and obvious corruption. It does not yet provide signed artifact authenticity.

### Not a security sandbox

The current filesystem sandbox should be read as a reproducibility feature, not a hostile-code containment feature.

It helps define inputs and stage snapshots, but it does not replace:

- containers
- seccomp
- Linux namespaces
- AppArmor or SELinux
- macOS sandbox profiles
- a VM boundary

## Guidance for Users

- treat `allow` as the declared deterministic input set
- prefer explicit argv commands over shell strings
- avoid relying on ambient network, time, or machine-specific host state
- run `repro check` when validating flaky or suspicious workloads

## Guidance for Contributors

Changes in these areas should always include regression tests:

- cache path handling
- filesystem boundary logic
- timeout cleanup
- config parsing and interpolation
- diff and determinism comparison behavior
