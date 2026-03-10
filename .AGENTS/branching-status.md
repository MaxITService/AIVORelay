# Branching Status

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> `Microsoft-store` sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## Microsoft-store

Last synced commit from `main`: `6e8bb7a` (propagated as `d34378e`) — fix(connector): rollback export and harden remote stt state.
Propagation was re-run from the correct later sync point on 2026-03-10 after restoring the branch to `9834519`, then completed through the remote STT / connector fix stack later the same day.
Note: `git cherry` may still show some older `main` commits because of historical patch-id/documentation divergence, not because this branch is missing the current propagated content.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.
