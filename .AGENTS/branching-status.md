# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `1b3077b6` — chore: bump version to 1.0.52.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor covers the reviewed source state reflected in Store content, including documented intentional exclusions; docs-only cursor updates do not advance it and excluded commits are not pending work.
Current alignment (2026-11-03): source consolidation, numeric version 1.0.52, approved English/Russian What's New and regenerated branch-specific bindings are committed. Main bdfee1b8 maps to Store c14e108f; main 1b3077b6 maps to Store cf4dc08d. Startup duplicate-shortcut migration 676192c6 maps to Store 387362f6 before the respective version bumps; only failed bindings conflicting with registered actions are cleared, with retained localized startup toasts. Both Windows x64 debug Tauri builds (including TypeScript and Vite) passed on Rust 1.99.1 after debug startup regenerated bindings. No tests, installers, signing, tags or pushes were run. Main-only README and release notes remain excluded; Store AVX2, updater 2.10.0 and packaging boundaries are preserved.
Alignment note: On 2026-10-26, 68 code/build commits through 1.0.51 bump c59c4173 -> 443efedd were propagated; docs, Nix, updater and main-only CI were excluded, and the Store release workflows were removed because main's release.yml now builds this branch into the shared `vX.Y.Z` draft. Earlier: on 2026-10-16, 32 runtime/UI commits after 1.0.49 were propagated through 62c7d98a — Update the local speech engine to transcribe-cpp 0.4.0 — mapping to 58081012. Main release preparation 24c0841c — chore: bump version to 1.0.50 — maps to Store ba18a2f8, followed only by the bindings fix 8043e997 -> b22f9618 and build fix e5d52377 -> 24e3d8c6, with matching numeric versions and English/Russian What's New since 1.0.49. Main-only README, sync, code, TESTING and build-environment notes were excluded. Pitch tests and Gemini final-word grace were adapted around existing Store exclusions; network tests remain excluded. The emergency updater exclusion and Store packaging, signing, AVX2 and dependency differences remain intact. The Store lockfile was regenerated locally; only the app version and transcribe-cpp changed.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is Store-incompatible. Exclude self-update/auto-update changes, including the emergency crash-screen updater, and AVX512-only changes; AVX2 is allowed. Adapt later commits that touch excluded features without restoring them.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
