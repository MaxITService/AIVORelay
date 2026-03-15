# Branching Status

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## Microsoft-store

Last synced commit from `main`: `ca08fe72` — feat(settings): repair invalid settings and bump version to 1.0.3.
Note: the cursor always points to the last propagated commit, not the cursor-update commit itself (to avoid a circular hash dependency).
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## cuda-integration

Last synced commit from `main`: `ca08fe72` — feat(settings): repair invalid settings and bump version to 1.0.3.
Note: the cursor always points to the last propagated commit from `main`, not to CUDA-only commits that were created on top of it.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for CUDA Edition propagation, bring over the intended `main` commit set in order unless a commit is CUDA-incompatible. Default exclusions are Microsoft Store-specific changes and branch-local CUDA dependency/release wiring that only exists on `cuda-integration`.
