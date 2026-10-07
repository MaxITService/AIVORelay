# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `c5df5ef5` — chore: bump version to 1.0.46.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor always points to the last propagated `main` state reflected in branch content, not to a docs-only cursor-update commit itself.
Alignment note: runtime/UI and 1.0.46 release preparation propagated through the cursor above. Main-only code notes, testing documentation, standalone frontend tests, certificate installation tooling and Windows signing changes are excluded. The updater-search commit is excluded. Existing documentation and test-support differences are preserved. Store config, workflows, updater, and AVX2 rules are preserved. Store lockfile app version was updated independently. Release notes and What's New describe changes since published 1.0.45. The user confirmed build verification; no agent build/tests were run.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
