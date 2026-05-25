# GitHub Setup

Use this document when pushing the repository to GitHub for the first time or when hardening the hosted repository configuration.

## 1. Create the Remote Repository

- create `reprorun` on GitHub
- keep the repository empty
- do not generate a remote README, license, or `.gitignore`

## 2. Update Local Placeholders

Before pushing, replace placeholders in:

- `.github/CODEOWNERS`
- `.github/ISSUE_TEMPLATE/config.yml`
- workspace metadata in `Cargo.toml`
  - `repository`
  - `homepage`
  - `documentation`

## 3. Connect the Remote and Push

```powershell
git remote add origin https://github.com/<org-or-user>/reprorun.git
git push -u origin main
```

If your local branch is not already `main`:

```powershell
git branch -M main
git push -u origin main
```

## 4. Review Repository Files

Current GitHub-specific files already included:

- `.github/CODEOWNERS`
- `.github/PULL_REQUEST_TEMPLATE.md`
- `.github/ISSUE_TEMPLATE/*`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

## 5. Configure Repository Settings

Recommended settings:

- require pull requests for changes to the default branch
- require CI status checks before merge
- require at least one review
- enable branch protection for `main`
- enable vulnerability alerts and Dependabot where appropriate

## 6. First Release

Once the repository is pushed and CI is green:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

That triggers the release workflow and uploads packaged binaries.

## 7. Ongoing Maintenance

When adding major behavior:

- update docs under `docs/`
- update examples if user-facing config changes
- add regression tests
- keep workflows and issue templates aligned with current project behavior
