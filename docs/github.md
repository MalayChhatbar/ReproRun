# GitHub Setup

Use this checklist when creating the remote repository.

## 1. Create the GitHub repository

- Create `reprorun` on GitHub.
- Keep it empty (no README/license generated remotely).

## 2. Connect local repo and push

```powershell
git remote add origin https://github.com/<org-or-user>/reprorun.git
git push -u origin master
```

If you prefer `main`:

```powershell
git branch -M main
git push -u origin main
```

## 3. Update placeholders

Replace placeholders in:

- `.github/CODEOWNERS`
- `.github/ISSUE_TEMPLATE/config.yml`
- workspace metadata in `Cargo.toml` (`repository`, `homepage`, `documentation`)

## 4. Enable protections

- Branch protection on default branch.
- Require status checks from CI workflows.
- Require pull request reviews.

## 5. Releases

Create a tag to trigger release workflow:

```powershell
git tag v0.1.0
git push origin v0.1.0
```
