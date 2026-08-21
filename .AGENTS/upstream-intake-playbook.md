# Upstream Intake Playbook
Branch tags: #branch/main

This playbook is only for `upstream -> main`.

Do not use this file for `main -> release/microsoft-store` propagation.
That is a separate workflow described in:
- [[.AGENTS/main-to-microsoft-store-propagation-playbook|main-to-microsoft-store-propagation-playbook.md]]

## Scope

- Source branch: `upstream/main`
- Target branch: `main`
- During this flow, stay on `main` unless the user explicitly requests a later propagation step.
- This file is maintained from `main` only.
- Do not keep or update branch-local copies of this file in non-`main` worktrees.

## Tracking

Primary reference:
- [[.AGENTS/upstream-sync-log|upstream-sync-log.md]]

Optional context only:
- [[.AGENTS/branching-status|branching-status.md]]
- local machine path hints in [[.AGENTS/.untracked/local-paths|local-paths.md]] when available
- `Q:\Handy-upstream` when present: local read-only checkout of `cjpais/Handy` for source comparison; refresh it before relying on current upstream code.

Authoritative source:
- [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]] is maintained from `main` only.
- [[.AGENTS/upstream-sync-log|upstream-sync-log.md]] is maintained from `main` only.
- Non-`main` branches must not carry independent copies of these upstream intake docs.

Useful commands:

```bash
git fetch upstream
# take <last_upstream_sha> from upstream-sync-log.md
git log <last_upstream_sha>..upstream/main --oneline
git cherry -v main upstream/main
```

Review-cursor safeguard:
- Do not advance the safe review cursor past a merge commit until every parent in the corridor and the merge resolution have been reviewed and classified.

## Selection Rules

Take:
- Windows-relevant runtime fixes
- hotkey/input/shortcut fixes
- STT/transcription/core audio pipeline fixes
- dependency/security updates used by active Windows code paths
- tray/UI fixes that affect Windows behavior

Optional:
- pure translations
- small UX improvements with limited risk
- partial-value changes with high conflict surface

Skip:
- release-only bumps/tags
- sponsor/template/document-only housekeeping
- Linux/macOS-only runtime changes unless they contain shared critical fixes

## Merge Commits

Merge commits are not direct cherry-pick candidates by default, but they are never assumed to be content-free or automatically skipped. Inspect every merge's parents, topology, combined diff, remerge diff, final tree, and conflict-resolution behavior. Judge relevance and fork fitness with the same Windows/fork selection rules used for ordinary commits.

Inspect especially carefully when selected or relevant parent commits touch overlapping files, or when an intermediate parent is broken or incomplete. If the resolution contains relevant behavior, manually adapt it from the final merge tree (or an exact minimal diff) and record the merge SHA as reviewed/reference in [[.AGENTS/upstream-sync-log|upstream-sync-log.md]]. Skip a merge only after confirming that it contains no relevant resolution-only behavior.

Useful merge inspection commands:

```bash
git show --no-patch --format='%H%n%P%n%s' <merge_sha>
git show --cc <merge_sha>
git show --remerge-diff <merge_sha>
git diff <merge_sha>^1..<merge_sha>
git diff <merge_sha>^2..<merge_sha>
```

Octopus merges or merges with additional parents require equivalent inspection of each parent.

## Workflow

1. Confirm working tree status and starting branch.
2. Switch to `main`.
3. Cherry-pick selected ordinary upstream commits one by one. For a selected merge resolution, manually adapt the reviewed final-tree behavior or exact minimal diff instead of cherry-picking the merge by default.
4. If conflicts are small and safe, resolve and continue.
5. If conflicts are many/high-risk, run `git cherry-pick --abort` and switch to diff-path using `.AGENTS/.untracked/<sha>.diff.txt`.
6. Record the integrated upstream commit SHA and the intended `main` commit message for each taken item.
7. Update [[.AGENTS/upstream-sync-log|upstream-sync-log.md]].
8. At the end of intake, show the remaining working tree status and propose a commit plan that leaves the worktree clean after the work. Call out any pre-existing unrelated changes before including them in that plan.
9. If the user later wants `main -> release/microsoft-store`, stop using this playbook and switch to the branch propagation playbook.

## Cargo.lock

Never cherry-pick `src-tauri/Cargo.lock` from upstream directly.

If conflict occurs:

```bash
git checkout --ours src-tauri/Cargo.lock
git add src-tauri/Cargo.lock
```
