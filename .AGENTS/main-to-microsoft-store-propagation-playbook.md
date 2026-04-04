# Main To Microsoft-store Propagation Playbook
Branch tags: #branch/main #branch/microsoft-store

This playbook is only for `main -> Microsoft-store`.

Do not use upstream-intake filtering here.
That is a separate workflow described in:
- [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]]

Primary rolling reference:
- [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]

## Scope

- Source branch: `main`
- Target branch: `Microsoft-store`
- Run only when the user explicitly requests propagation.

## Start Point Rule

Before picking the first commit, verify the real latest `main`-derived commit already present in `Microsoft-store` with git history.

Use:

```bash
git log Microsoft-store --oneline
git cherry -v Microsoft-store main
git reflog Microsoft-store
```

Optional remote cross-check after fetch:

```bash
git log origin/Microsoft-store --oneline
git cherry -v origin/Microsoft-store main
```

Do not trust `branching-status.md` alone as the propagation cursor.
Treat it as a quick reference that must match git history.

## Propagation Scope

Default behavior:
- propagate the intended `main` commit set in order

Default exclusions for Microsoft Store Edition:
- documentation changes
- self-update / auto-update changes
- AVX512-only changes

Allowed by default:
- AVX2-targeted changes

Documentation handling:
- exclude documentation changes from the normal propagation set by default
- if a documentation file is clearly `main`-only, do not propagate it
- if a documentation file appears applicable to multiple branches and looks like a file that should exist on all branches, stop and ask the user about that specific file before including it

Never cherry-pick directly from `upstream` into `Microsoft-store`.
Always propagate from `main`.

## Workflow

1. Confirm working tree status and remember starting branch.
2. Verify the real target-branch sync point with git history.
3. Review documentation changes separately from code changes.
4. Exclude any documentation file that is clearly `main`-only.
5. If a documentation file appears branch-shared and looks like it should exist on all branches, ask the user about that specific file before propagating it.
6. Switch to `Microsoft-store`.
7. Cherry-pick selected non-documentation `main` commits in order, plus only the documentation files the user explicitly approved.
8. If conflicts are small and safe, resolve and continue.
9. If conflicts are many/high-risk, run `git cherry-pick --abort` and switch to diff-path using `.AGENTS/.UNTRACKED/<sha>.diff.txt`.
10. Record resulting local commit hashes.
11. Update [[.AGENTS/branch-propagation-log|branch-propagation-log.md]] in the target branch worktree after successful propagation.
12. Update [[.AGENTS/branching-status|branching-status.md]] in the target branch worktree after successful propagation.
13. Mirror the same propagation-log entry and cursor update back into `main`'s `.AGENTS` docs before considering the sync finished.
14. Return to the original branch if needed.

## Cargo.lock

Never cherry-pick `src-tauri/Cargo.lock` directly during propagation.

If conflict occurs:

```bash
git checkout --ours src-tauri/Cargo.lock
git add src-tauri/Cargo.lock
```
