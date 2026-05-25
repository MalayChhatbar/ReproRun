# ReproRun Documentation

This directory documents the current MVP implementation, not an aspirational future design. If a behavior is described here, it should match the current code and tests.

## Core Docs

- [architecture.md](architecture.md): workspace structure, crate boundaries, execution flow, and artifact lifecycle
- [configuration.md](configuration.md): strict YAML schema, defaults, interpolation behavior, and validation rules
- [cli.md](cli.md): command reference, exit behavior, output modes, and examples
- [determinism.md](determinism.md): what ReproRun controls, what is hashed, and what remains best-effort

## Operational Docs

- [testing.md](testing.md): test philosophy, coverage areas, current regression suites, and validation commands
- [security.md](security.md): trust boundaries, hardening currently implemented, and remaining limitations
- [development.md](development.md): local workflow, coding conventions, and expected validation steps
- [release.md](release.md): versioning and GitHub release flow
- [github.md](github.md): repository setup, placeholders to replace, and branch protection guidance

## Suggested Reading Order

1. Start with [architecture.md](architecture.md).
2. Read [configuration.md](configuration.md) and [cli.md](cli.md) if you want to use the tool.
3. Read [determinism.md](determinism.md) and [security.md](security.md) if you need to understand guarantees and limits.
4. Read [testing.md](testing.md) and [development.md](development.md) before contributing code.
