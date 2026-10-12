# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `57f9d077` — fix(settings): explain risks of administrator autostart.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor always points to the last propagated `main` state reflected in branch content, not to a docs-only cursor-update commit itself.
Alignment note: The verified 71d9675a cursor already reflected 22 runtime/UI commits despite rewritten main history. Six later runtime/test commits now add model-deletion test coverage, interactive wide tooltips, independent profile output and protected live delivery, collapsed profile summaries and reliable copying, Help navigation, and administrator-autostart guidance. Store inline Gemini finalization keeps the protected-live-output and early-opening-key guards. Main-only code-notes and docs-only a6e81234/2914a9dc are reviewed exclusions; docs/help-sections.md was explicitly approved and synced from main 384b343f. Version 1.0.48 and localized release highlights are prepared in 55639c5a — chore: bump version to 1.0.48. Store packaging, AVX2, updater and dependencies remain unchanged; only the Store lock own-package version was edited. No lock regeneration or agent build/tests were run.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
