# Branching Status

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## Microsoft-store

Last synced commit from `main`: `9da5f76f` — feat(history): save recordings before transcription.
Maintenance rule: after a successful `main` -> `Microsoft-store` propagation, update this main-copy cursor and the `Microsoft-store` worktree copy together.
Note: the cursor always points to the last propagated `main` state reflected in branch content, not to a docs-only cursor-update commit itself.
Alignment note: `Microsoft-store` now matches `main` for all non-Store-specific files via force overwrite. Intentional differences remain only for Microsoft Store-specific docs/config/workflow/updater/AVX2 files.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## cuda-integration

Last synced commit from `main`: `ffb30103` — docs(sync): advance upstream intake log to 58cda3f3.
Maintenance rule: after a successful `main` -> `cuda-integration` propagation, update this main-copy cursor and the `cuda-integration` worktree copy together.
Note: the cursor always points to the last propagated commit from `main`, not to CUDA-only commits that were created on top of it.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for CUDA Edition propagation, bring over the intended `main` commit set in order unless a commit is CUDA-incompatible. Default exclusions are Microsoft Store-specific changes and branch-local CUDA dependency/release wiring that only exists on `cuda-integration`.
