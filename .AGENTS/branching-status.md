Branch tags: #branch/codex-combined

# Branching Status

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## codex/combined

Last synced commit from `main`: `5752185f` — change default model unload timeout to 15 minutes.
Maintenance rule: after a successful `main` -> `codex/combined` propagation, update this file and the local propagation log.
Note: the cursor always points to the last propagated commit from `main`, not to combined-only commits created on top of it.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Combined Edition propagation, bring over intended `main` runtime fixes in order unless a commit conflicts with combined packaging or variant-launcher wiring.
