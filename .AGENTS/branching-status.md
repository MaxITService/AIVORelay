# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `367a6af1` — Keep hidden frontend sourcemaps as a CI artifact.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor covers the reviewed source state reflected in Store content, including documented intentional exclusions; docs-only cursor updates do not advance it and excluded commits are not pending work.
Alignment note: On 2026-10-15, 17 ordered code/CI commits after a7120724 were propagated through 367a6af1, mapped to Store 0a2f1e84 — Keep hidden frontend sourcemaps as a CI artifact. Emergency crash-screen updater 927586a9 is intentionally excluded; fa5a1bee and 02dbc8e4 retain shared logging and overlay recovery without updater imports, UI, or optional updater boundaries. Main-only sync/code notes were excluded; shared docs/help-sections.md was approved and synced. Store render recovery and diagnostics now include separate hidden CI sourcemaps outside the packaged app. Store packaging, release tags/signing, AVX2, existing updater behavior, dependencies, lockfiles, and inline Gemini finalization are preserved. A read-only final diff audit was performed; no build or tests were run.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is Store-incompatible. Exclude self-update/auto-update changes, including the emergency crash-screen updater, and AVX512-only changes; AVX2 is allowed. Adapt later commits that touch excluded features without restoring them.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
