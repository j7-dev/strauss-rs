---
name: release
description: "Release strauss-rs: bump version in Cargo.toml, commit, tag, push to trigger cross-platform GitHub Actions build. Usage: /release [version]"
allowed-tools: Bash, Read, Edit
---

# Release strauss-rs

Automate the full release flow: version bump -> commit -> tag -> push.
The push triggers the GitHub Actions `release.yml` workflow which builds cross-platform binaries and creates a GitHub Release.

## Workflow

### Step 1: Determine version

- If the user provides a version argument (e.g., `/release 0.2.0`), use that version.
- If no argument is provided:
  1. Read `Cargo.toml` and extract the current `version` value.
  2. Parse as semver (MAJOR.MINOR.PATCH).
  3. Suggest next patch version (PATCH + 1) and ask the user to confirm or provide an alternative.

### Step 2: Validate

Before proceeding, check ALL of the following. If any check fails, STOP and inform the user:

1. **Working tree is clean**: Run `git status --porcelain`. If output is non-empty, STOP — user must commit or stash first.
2. **Tag does not exist**: Run `git tag -l "v{version}"`. If the tag exists, STOP — that version is already released.
3. **Branch check**: Run `git branch --show-current`. Warn if not on `master` but allow the user to proceed if they confirm.

### Step 3: Update Cargo.toml

Use the Edit tool to change the `version = "..."` line in `Cargo.toml` to the new version.

### Step 4: Update Cargo.lock

Run `cargo check` to update `Cargo.lock` with the new version. This is necessary because Cargo.lock embeds the package version.

```bash
cargo check
```

### Step 5: Commit

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to {version}"
```

### Step 6: Create tag

```bash
git tag "v{version}"
```

### Step 7: Confirm and Push

**IMPORTANT**: Before pushing, ask the user to confirm. Pushing triggers the GitHub Actions workflow and is irreversible.

After confirmation:

```bash
git push origin master --follow-tags
```

### Step 8: Report

Print a summary:

```
Release v{version} initiated!

- Commit: chore: bump version to {version}
- Tag: v{version}
- GitHub Actions: https://github.com/j7-dev/strauss-rs/actions
- Release page: https://github.com/j7-dev/strauss-rs/releases/tag/v{version}

GitHub Actions is now building binaries for Linux, macOS (Intel + Apple Silicon), and Windows.
Once complete, the release will appear with downloadable binaries.
```

## Error Handling

- If `git push` fails (e.g., network error), tell the user to retry: `git push origin master --follow-tags`
- If the user wants to undo before pushing, provide: `git tag -d v{version} && git reset --soft HEAD~1`
- Never force-push. Never delete remote tags automatically.

## Important Notes

- The version argument should NOT include the `v` prefix (e.g., `0.2.0` not `v0.2.0`). The `v` prefix is added only for git tags.
- Always confirm with the user before pushing.
- If version matches current Cargo.toml version, skip the version bump and just tag + push.
