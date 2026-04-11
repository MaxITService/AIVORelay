Branch tags: #branch/release-microsoft-store

# Store Sync Maintenance

Short local note for maintaining Microsoft Store branch sync docs.
This branch does not keep a full propagation log or global status. `main` tracks all branches.

## After A Successful Sync

Record the successful sync on `main` (update `branch-propagation-log.md` and `branching-status.md` in the `main` branch).

## Verification Rule

Before changing the sync cursor on `main`, verify the target branch with git history.
Useful commands:

```bash
git log release/microsoft-store --oneline
git cherry -v release/microsoft-store main
git reflog release/microsoft-store
```
