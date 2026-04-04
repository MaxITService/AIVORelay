# Main To codex/combined Propagation Playbook
Branch tags: #branch/main #branch/codex-combined

This playbook is only for `main -> codex/combined`.

Do not use upstream-intake filtering here.
That is a separate workflow described in:
- [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]]

Primary rolling reference:
- [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]

## Scope

- Source branch: `main`
- Target branch: `codex/combined`
- Run only when the user explicitly requests propagation.

## Start Point Rule

Before picking the first commit, verify the real latest `main`-derived commit already present in `codex/combined` with git history.

Use:

```bash
git log codex/combined --oneline
git cherry -v codex/combined main
git reflog codex/combined
```

Optional remote cross-check after fetch:

```bash
git log origin/codex/combined --oneline
git cherry -v origin/codex/combined main
```

Do not trust `branching-status.md` alone as the propagation cursor.
Treat it as a quick reference that must match git history.

## Propagation Scope

Default behavior:
- propagate the intended `main` commit set in order

Default exclusions for Combined Edition:
- documentation changes
- Microsoft Store-only updater policy changes
- CUDA-only dependency/release wiring
- branch-local multi-exe packaging changes that exist only on `codex/combined`

Allowed by default:
- shared runtime fixes from `main`
- UI fixes from `main`
- settings fixes from `main`

Documentation handling:
- exclude documentation changes from the normal propagation set by default
- if a documentation file is clearly `main`-only, do not propagate it
- if a documentation file appears applicable to multiple branches and looks like a file that should exist on all branches, stop and ask the user about that specific file before including it

Never cherry-pick directly from `upstream` into `codex/combined`.
Always propagate from `main`.

## Workflow

1. Confirm working tree status and remember starting branch.
2. Verify the real target-branch sync point with git history.
3. Review documentation changes separately from code changes.
4. Exclude any documentation file that is clearly `main`-only.
5. If a documentation file appears branch-shared and looks like it should exist on all branches, ask the user about that specific file before propagating it.
6. Switch to `codex/combined`.
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
