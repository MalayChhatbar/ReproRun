# Release Process

ReproRun uses semantic versioning and a tag-triggered GitHub release workflow.

## Versioning

- version numbers follow SemVer
- release tags use the form `vX.Y.Z`

## Build Metadata

The CLI embeds build-time metadata in version output:

- package version
- full git SHA
- short git SHA
- exact git tag when available
- dirty state at build time

This means a release build can be traced back to its source revision directly from `repro --version`.

## CI Workflow

Main validation workflow:

- `.github/workflows/ci.yml`

It runs:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo audit`

## Release Workflow

Release workflow:

- `.github/workflows/release.yml`

Trigger:

- push a tag matching `v*`

Current build matrix:

- Linux
- Windows
- macOS

Current release artifacts:

- `.tar.gz` for Unix targets
- `.zip` for Windows targets

## Recommended Release Checklist

1. Ensure the default branch is green in CI.
2. Review `CHANGELOG.md`.
3. Confirm documentation is current.
4. Confirm examples still work.
5. Tag the release:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

6. Verify that GitHub published:
   - release notes
   - packaged binaries for all matrix targets

## Notes

- build metadata is resolved at compile time
- a dirty working tree at build time is reflected in version output
- release builds should therefore be produced from a clean checkout
