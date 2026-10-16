# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `8043e997` — Regenerate TypeScript bindings for live stream insert and streamed benchmark settings.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor covers the reviewed source state reflected in Store content, including documented intentional exclusions; docs-only cursor updates do not advance it and excluded commits are not pending work.
Alignment note: On 2026-10-16, 32 runtime/UI commits after 1.0.49 were propagated through 62c7d98a — Update the local speech engine to transcribe-cpp 0.4.0 — mapping to 58081012. Main release preparation 24c0841c — chore: bump version to 1.0.50 — maps to Store ba18a2f8, followed only by the bindings fix 8043e997 -> b22f9618, with matching numeric versions and English/Russian What's New since 1.0.49. Main-only README, sync, code, TESTING and build-environment notes were excluded. Pitch tests and Gemini final-word grace were adapted around existing Store exclusions; network tests remain excluded. The emergency updater exclusion and Store packaging, signing, AVX2 and dependency differences remain intact. The Store lockfile was regenerated locally; only the app version and transcribe-cpp changed.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is Store-incompatible. Exclude self-update/auto-update changes, including the emergency crash-screen updater, and AVX512-only changes; AVX2 is allowed. Adapt later commits that touch excluded features without restoring them.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
