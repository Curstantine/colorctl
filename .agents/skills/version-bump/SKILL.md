---
name: version-bump
description: Cut a new semver release for colorctl — review commits since the last tag, propose a version bump, update the package version, and create an annotated tag.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: release
  opencode/slash: "true"
  opencode/autoinvoke: "false"
---

## What I do

- Find the last release tag (`vX.Y.Z`) and review commits since it.
- Recommend a semver bump (major / minor / patch) based on conventional commits.
- Update the package version, refresh `Cargo.lock`, commit it, generate a tag body in the repo's release style, and create an annotated GPG-signed tag.
- Push only with explicit user approval.

## When to use me

Use this when the user asks to bump the version, cut a release, or create a release tag (e.g. `/version-bump`, "release a new version", "tag v0.2.0").

## Workflow

Follow these steps in order. Stop and ask at each approval gate — never skip them.

### 1. Determine the baseline

1. Find the last tag:
   ```bash
   git tag --list --sort=-v:refname | head -5
   git describe --tags --abbrev=0
   ```
2. Read the current version from the root `Cargo.toml` (`[package] version`).
3. List commits since the last tag, with hashes and full messages:
   ```bash
   git log <LAST_TAG>..HEAD --oneline
   git log <LAST_TAG>..HEAD --format='%h %s%n%b' --no-merges
   ```
4. Check working tree state with `git status --short`. If dirty, tell the user and stop — ask them to commit or stash first.
5. If `HEAD` is already tagged (no commits since last tag), tell the user and stop.

### 2. Ask for the bump type (approval gate 1)

1. Analyze the commits using semver + conventional commits:
   - Breaking change (`!`, `BREAKING CHANGE`, removed API): **major** (`X+1.0.0`).
   - New feature (`feat:`): **minor** (`X.Y+1.0`, patch reset to 0).
   - Only fixes/perf/docs/refactor/chore (`fix:`, `perf:`, `docs:`, etc.): **patch** (`X.Y.Z+1`).
2. Use the `question` tool to ask the user which bump they want. Offer `minor (Recommended)` first when features are present, otherwise `patch (Recommended)`, plus `major` and the exact computed versions (e.g. `v0.1.2` → patch `v0.1.3`, minor `v0.2.0`, major `v1.0.0`).
3. Compute the new version from the chosen bump. Tags are always `v<semver>` (e.g. `v0.2.0`).

### 3. Bump the package version

1. Edit the root `Cargo.toml` `[package] version` to the new version (without the `v` prefix).
2. Regenerate `Cargo.lock`:
   ```bash
   cargo check
   ```
   This project builds on Linux only, so run this inside `nix develop` (or another Linux environment) if needed.
3. Verify with `git diff --stat` — expect `Cargo.toml` plus the `colorctl` version line in `Cargo.lock`.
4. Commit (GPG-signed, matching repo history):
   ```bash
   git add Cargo.toml Cargo.lock
   git commit -S -m "chore: Bump version to <NEW_VERSION>"
   ```
5. If the commit fails (e.g. signing or hooks), stop and report — do not continue to tagging.

### 4. Generate the tag body

Draft the annotated-tag message in the repo's established style (see `git tag -l --format='%(contents:subject)%0a%0a%(contents:body)' v0.1.1` for reference):

```text
Release v<NEW>

Highlights since v<OLD>
- 2–4 user-facing bullets describing what matters (not every commit).

<Section per change type, only if non-empty>
- Short imperative summary (<short-hash>)
...

Full changelog: https://github.com/Curstantine/colorctl/compare/v<OLD>...v<NEW>
```

Rules for the body:

- Group commits by type: `Features` (`feat`), `Fixed` (`fix`), `Performance` (`perf`), `Changed` (refactor, style, breaking-adjacent), `Docs`/`Chores` only if user-visible — skip pure `chore: clippy`-style noise unless it is the only change.
- One line per significant commit: short imperative summary + short hash in parentheses, e.g. `- Add versioned release assets and Debian packaging (c4f4d70)`.
- Highlights are hand-written, user-facing, max 4 bullets. Never paste raw `git log` output.
- Keep the `Full changelog: .../compare/v<OLD>...v<NEW>` trailer exact — the release workflow reuses this body verbatim.
- Show the draft body to the user and ask for approval/edits before creating the tag.

### 5. Create the tag (approval gate 2)

1. Only after the user approves the version **and** the body, create an annotated signed tag on the bump commit:
   ```bash
   git tag -s "v<NEW>" -m "<TAG_BODY>"
   ```
   Preserve newlines in `<TAG_BODY>` (heredoc or `-F <file>`). Never use a lightweight tag.
2. Verify the tag has no signature block leakage and matches the draft:
   ```bash
   git tag -l --format='%(contents:subject)%0a%0a%(contents:body)' "v<NEW>"
   ```
3. Report the tag name, commit, and body to the user.

### 6. Push (approval gate 3)

1. After the tag is created, use the `question` tool to ask whether to push. Default to **not** pushing. Options:
   - Push tag only: `git push origin "v<NEW>"`
   - Push branch + tag: `git push origin <BRANCH> "v<NEW>"` (state which branch).
   - Don't push.
2. Never run `git push` without an explicit yes from this step — not even tag-only.
3. Remind the user that pushing the tag triggers `.github/workflows/release.yml`, which builds the artifacts and creates the GitHub Release using the tag message as its body.

## Strict rules

- Tags are always annotated + signed (`git tag -s`), format `vX.Y.Z`. Never amend or force-move an existing tag.
- Never push without explicit approval from step 6, even if the user approved the tag in step 5.
- Never invent commits or hashes — every body line must trace to a real commit in `<LAST_TAG>..HEAD` plus the bump commit.
- If there are zero commits since the last tag, stop instead of creating an empty release.
