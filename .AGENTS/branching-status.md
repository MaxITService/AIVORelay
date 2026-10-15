# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `ba4d88dd` — chore: bump version to 1.0.49.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor covers the reviewed source state reflected in Store content, including documented intentional exclusions; docs-only cursor updates do not advance it and excluded commits are not pending work.
Alignment note: On 2026-10-15, 0771426a — Let new profiles get a shortcut and explain when one is needed — was propagated to ac8ce4fe. Release preparation ba4d88dd — chore: bump version to 1.0.49 — maps to Store 1cc227d5 with matching numeric versions and English/Russian What's New since 1.0.48. The main release notes retain the emergency update feature; Store notes omit it and retain Store notices. Main-only 69534cd8 sync documentation was excluded. The earlier 927586a9 emergency updater exclusion and adapted logging/overlay recovery remain intact. Store packaging, signing, AVX2, dependencies, and inline Gemini finalization are preserved. Each branch's own Cargo.lock was regenerated and checked with offline and locked metadata; only the app package version changed. Final source/target and whitespace diffs were reviewed; no local build or tests were run.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is Store-incompatible. Exclude self-update/auto-update changes, including the emergency crash-screen updater, and AVX512-only changes; AVX2 is allowed. Adapt later commits that touch excluded features without restoring them.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
