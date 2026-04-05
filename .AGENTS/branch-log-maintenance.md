Branch tags: #branch/codex-combined

# Combined Sync Maintenance

Read this file only when recording a completed `main -> codex/combined` sync.
This branch does not keep a full propagation log or global status. `main` tracks all branches.

## After A Successful Sync

1. Refresh [[.AGENTS/combined-branch-notes|combined-branch-notes.md]] if the branch-local file set changed.
2. Record the successful sync on `main` (update `branch-propagation-log.md` and `branching-status.md` in the `main` branch).

## Verification Rule

Before writing the cursor, verify the real sync point with git history.
Do not trust docs alone as the source of truth.

Useful commands:

```bash
git log codex/combined --oneline
git cherry -v codex/combined main
git reflog codex/combined
```
