# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `827ae3da` — fix(settings): separate history controls from entries.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor always points to the last propagated `main` state reflected in branch content, not to a docs-only cursor-update commit itself.
Alignment note: `release/microsoft-store` now matches `main` for all non-Store-specific files via force overwrite. Intentional differences remain only for Microsoft Store-specific docs/config/workflow/updater/AVX2 files.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## integration/cuda

Last synced commit from `main`: `827ae3da` — fix(settings): separate history controls from entries.
Maintenance rule: after a successful `main` -> `integration/cuda` propagation, update this main-copy cursor and the `integration/cuda` worktree copy together.
Note: the cursor always points to the last propagated commit from `main`, not to CUDA-only commits that were created on top of it.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for CUDA Edition propagation, bring over the intended `main` commit set in order unless a commit is CUDA-incompatible. Default exclusions are Microsoft Store-specific changes and branch-local CUDA dependency/release wiring that only exists on `integration/cuda`.
Operational note (2026-03-23): for content documentation, treat the branch as reset to the `8c52c9f0` baseline and describe only the remaining CUDA/build/docs layer listed in the `integration/cuda` worktree copy of `[[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]]`.

## integration/combined

Last synced commit from `main`: `827ae3da` — fix(settings): separate history controls from entries.
Maintenance rule: after a successful `main` -> `integration/combined` propagation, update this main-copy cursor and the `integration/combined` worktree copy together.
Note: the cursor always points to the last propagated commit from `main`, not to combined-only commits created on top of it.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Combined Edition propagation, bring over intended `main` runtime fixes in order unless a commit conflicts with combined packaging or variant-launcher wiring.
