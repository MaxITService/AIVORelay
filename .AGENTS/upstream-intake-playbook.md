# Upstream Intake Playbook
Branch tags: #branch/main

This playbook is only for `upstream -> main`.

Do not use this file for `main -> release/microsoft-store` or `main -> integration/cuda` propagation.
That is a separate workflow described in:
- [[.AGENTS/main-to-microsoft-store-propagation-playbook|main-to-microsoft-store-propagation-playbook.md]]
- [[.AGENTS/main-to-cuda-propagation-playbook|main-to-cuda-propagation-playbook.md]]

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
- local machine path hints in [[.AGENTS/.UNTRACKED/local-paths|local-paths.md]] when available

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
- merge commits
- sponsor/template/document-only housekeeping
- Linux/macOS-only runtime changes unless they contain shared critical fixes

## Workflow

1. Confirm working tree status and starting branch.
2. Switch to `main`.
3. Cherry-pick selected upstream commits one by one.
4. If conflicts are small and safe, resolve and continue.
5. If conflicts are many/high-risk, run `git cherry-pick --abort` and switch to diff-path using `.AGENTS/.UNTRACKED/<sha>.diff.txt`.
6. Record the integrated upstream commit SHA and the intended `main` commit message for each taken item.
7. Update [[.AGENTS/upstream-sync-log|upstream-sync-log.md]].
8. At the end of intake, show the remaining working tree status and propose a commit plan that leaves the worktree clean after the work. Call out any pre-existing unrelated changes before including them in that plan.
9. If the user later wants `main -> release/microsoft-store` or `main -> integration/cuda`, stop using this playbook and switch to the branch propagation playbook.

## Cargo.lock

Never cherry-pick `src-tauri/Cargo.lock` from upstream directly.

If conflict occurs:

```bash
git checkout --ours src-tauri/Cargo.lock
git add src-tauri/Cargo.lock
```
