# Release Process

## Versioning

This project uses SemVer.

## Tag-Based Release

GitHub Actions workflow: `.github/workflows/release.yml`

Trigger:

- push a tag matching `v*` (for example `v0.1.0`)

Pipeline:

1. Build release binary for Linux, Windows, and macOS.
2. Package artifacts (`.tar.gz` / `.zip`).
3. Publish GitHub release with generated notes.

## Recommended Steps

1. Ensure CI is green on `main`.
2. Update `CHANGELOG.md`.
3. Create and push tag:

```powershell
git tag v0.1.0
git push origin v0.1.0
```
