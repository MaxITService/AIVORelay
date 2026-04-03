Branch tags: #branch/microsoft-store

# Branching Status

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## Microsoft-store

Last synced commit from `main`: `4d2750b5` — fix: accept upstream old CPU crash fix.
Maintenance rule: after a successful `main` -> `Microsoft-store` propagation, update this file and the local propagation log.
Note: the cursor always points to the last propagated `main` state reflected in branch content, not to a docs-only cursor-update commit itself.
Alignment note: non-Store-specific program documentation is owned by `main`.
Sync rule: source commits come from `main` only.
Propagation scope rule: bring over intended `main` commits in order unless a commit is Store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes. AVX2 is allowed.
