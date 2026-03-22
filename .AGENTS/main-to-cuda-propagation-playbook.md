# Main To cuda-integration Propagation Playbook

This playbook is only for `main -> cuda-integration`.

Do not use upstream-intake filtering here.
That is a separate workflow described in:
- [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]]

Primary rolling reference:
- [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]
- [[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]]

## Scope

- Source branch: `main`
- Target branch: `cuda-integration`
- Run only when the user explicitly requests propagation.

## Start Point Rule

Before picking the first commit, verify the real latest `main`-derived commit already present in `cuda-integration` with git history.

Use:

```bash
git log cuda-integration --oneline
git cherry -v cuda-integration main
git reflog cuda-integration
```

Optional remote cross-check after fetch:

```bash
git log origin/cuda-integration --oneline
git cherry -v origin/cuda-integration main
```

Do not trust `branching-status.md` alone as the propagation cursor.
Treat it as a quick reference that must match git history.

## Propagation Scope

Default behavior:
- propagate the intended `main` commit set in order

Default exclusions for CUDA Edition:
- Microsoft Store-specific changes
- branch-local CUDA dependency/release wiring changes that only exist on `cuda-integration`

Allowed by default:
- normal runtime fixes from `main`
- UI fixes from `main`
- branch-safe build/release documentation updates that apply to CUDA too

Never cherry-pick directly from `upstream` into `cuda-integration`.
Always propagate from `main`.

## Workflow

1. Confirm working tree status and remember starting branch.
2. Verify the real target-branch sync point with git history.
3. Switch to `cuda-integration`.
4. Cherry-pick selected `main` commits in order.
5. If conflicts are small and safe, resolve and continue.
6. If conflicts are many/high-risk, run `git cherry-pick --abort` and switch to diff-path using `.AGENTS/.UNTRACKED/<sha>.diff.txt`.
7. Record resulting local commit hashes.
8. Update [[.AGENTS/branch-propagation-log|branch-propagation-log.md]] in the target branch worktree after successful propagation.
9. Update [[.AGENTS/branching-status|branching-status.md]] in the target branch worktree after successful propagation.
10. Refresh [[CUDA]] and [[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]] so the documented file list still matches the real branch-local layer.
11. Mirror the same propagation-log entry and cursor update back into `main`'s `.AGENTS` docs before considering the sync finished.
12. Return to the original branch if needed.

## Cargo.lock

Never cherry-pick `src-tauri/Cargo.lock` directly during propagation.

If conflict occurs:

```bash
git checkout --ours src-tauri/Cargo.lock
git add src-tauri/Cargo.lock
```
