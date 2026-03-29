# Branching Status

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## Microsoft-store

Last synced commit from `main`: `9a15c63b` — fix: redact stored secrets in settings debug logs.
Maintenance rule: after a successful `main` -> `Microsoft-store` propagation, update this main-copy cursor and the `Microsoft-store` worktree copy together.
Note: the cursor always points to the last propagated commit, not the cursor-update commit itself (to avoid a circular hash dependency).
Doc maintenance rule: update this worktree copy and `main`'s `.AGENTS/branching-status.md` together after every successful propagation.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## cuda-integration

Last synced commit from `main`: `4725eca3` — fix(tray): show app version in tooltip.
Maintenance rule: after a successful `main` -> `cuda-integration` propagation, update this main-copy cursor and the `cuda-integration` worktree copy together.
Note: the cursor always points to the last propagated commit from `main`, not to CUDA-only commits that were created on top of it.
Doc maintenance rule: update this worktree copy and `main`'s `.AGENTS/branching-status.md` together after every successful propagation.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for CUDA Edition propagation, bring over the intended `main` commit set in order unless a commit is CUDA-incompatible. Default exclusions are Microsoft Store-specific changes and branch-local CUDA dependency/release wiring that only exists on `cuda-integration`.
Operational note (2026-03-23): for content documentation, treat the branch as reset to the `8c52c9f0` baseline and describe only the remaining CUDA/build/docs layer listed in [[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]].
