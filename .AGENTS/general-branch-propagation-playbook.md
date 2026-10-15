# General Branch Propagation Workflow
Branch tags: #branch/main #branch/release-microsoft-store

This playbook contains the common rules for propagating commits from `main` to any of the release branches. 
For branch-specific exclusions and allowed commit types, see their respective playbooks:
- [[.AGENTS/main-to-microsoft-store-propagation-playbook|main-to-microsoft-store-propagation-playbook.md]]

`integration/cuda` and `integration/combined` are frozen and are not propagation targets.

Do not use upstream-intake filtering here. That is a separate workflow described in and is for upstream only:
- [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]]

Primary rolling reference:
- [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]

## Start Point Rule

Before picking the first commit, verify the real latest `main`-derived commit already present in the target branch with git history.

Use (replace `TARGET_BRANCH` with actual branch name):

```bash
git log TARGET_BRANCH --oneline
git cherry -v TARGET_BRANCH main
git reflog TARGET_BRANCH
```

Optional remote cross-check after fetch:

```bash
git log origin/TARGET_BRANCH --oneline
git cherry -v origin/TARGET_BRANCH main
```

Do not trust `branching-status.md` alone as the propagation cursor.
Treat it as a quick reference that must match git history.

## General Propagation Rules

Default behavior:
- propagate the intended `main` commit set in order
- run only when the user explicitly requests propagation
- **Never** cherry-pick directly from `upstream` into release branches. Always propagate from `main`.

Common Documentation Handling:
- exclude documentation changes from the normal propagation set by default
- if a documentation file is clearly `main`-only, do not propagate it. Most docs are `main`-only.
- if a documentation file appears applicable to multiple branches and looks like a file that should exist on all branches, stop and ask the user about that specific file before including it

## Common Workflow

1. Confirm working tree status and remember starting branch.
2. Verify the real target-branch sync point with git history.
3. Review documentation changes separately from code changes.
4. Exclude any documentation file that is clearly `main`-only.
5. If a documentation file appears branch-shared and looks like it should exist on all branches, ask the user about that specific file before propagating it.
6. Check the target branch's specific playbook for additional exclusions/inclusions.
7. Present a numbered table of the intended commits, including exclusions and adaptations. Obtain approval unless the user has already explicitly instructed you to proceed with this selection. Approval means proceed with all non-rejected items.
8. Use the existing target-branch worktree when available; otherwise switch to the target branch.
9. Cherry-pick selected non-documentation `main` commits in order, plus only the documentation files the user explicitly approved, minus branch-specific exclusions.
10. If conflicts are small and safe, resolve and continue.
11. If conflicts are many/high-risk, run `git cherry-pick --abort` and switch to diff-path using `.AGENTS/.untracked/<sha>.diff.txt`.
12. Record resulting local commit hashes.
13. Update [[.AGENTS/branch-propagation-log|branch-propagation-log.md]] in the target branch worktree after successful propagation.
14. Update [[.AGENTS/branching-status|branching-status.md]] in the target branch worktree after successful propagation.
15. Mirror the same propagation-log entry and cursor update back into `main`'s `.AGENTS` docs before considering the sync finished.
16. Return to the original branch if needed.

## Mixed Commits And Excluded Features

- Review changed paths and hunks, not only commit titles. A later runtime fix can depend on an earlier excluded feature.
- For mixed code/documentation commits, apply only eligible code and explicitly approved documentation. Keep branch-local agent notes separate.
- After excluding a feature, inspect later edits to the same files and their imports, helpers, UI, permissions, and translations. Adapt those edits without restoring the excluded feature.
- Do not replace a branch-divergent file with the full `main` version to resolve a conflict; retain only the intended changes and the target's contract.
- Inspect the final source/target diff and confirm that protected configuration, dependencies, and excluded features retain their intended differences. This read-only audit does not replace builds or tests; run those only with user authorization.
- Record source-to-target commit provenance, excluded commits, adapted hunks, approved documentation, and verification limits. The cursor covers the reviewed source state, including documented exclusions; intentionally excluded commits are not pending work in the next propagation.

## Cargo.lock Rules

Never cherry-pick `src-tauri/Cargo.lock` directly during propagation.

If conflict occurs:

```bash
git checkout --ours src-tauri/Cargo.lock
git add src-tauri/Cargo.lock
```
