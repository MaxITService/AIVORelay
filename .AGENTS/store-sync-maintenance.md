Branch tags: #branch/microsoft-store

# Store Sync Maintenance

Short local note for maintaining Microsoft Store branch sync docs.

- `branching-status.md` is a quick reference, not the source of truth.
- Before changing the sync cursor, verify the branch with `git log`, `git cherry`, and if needed `git reflog`.
- After a successful `main` -> `Microsoft-store` propagation, update:
  - [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]
  - [[.AGENTS/branching-status|branching-status.md]]
- Mirror the same propagation row and cursor update back to `main`.
- Keep notes short.
