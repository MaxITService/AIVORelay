# Branching Status
Branch tags: #branch/main #branch/release-microsoft-store

Operational note: this file is a quick reference, not the sole source of truth for the next propagation start point.
Before starting a new `main` -> branch sync, verify the target branch directly with git history (`git log`, `git cherry`, and if needed `git reflog`).

## release/microsoft-store

Last synced commit from `main`: `7527485d` — chore: bump version to 1.0.47.
Maintenance rule: after a successful `main` -> `release/microsoft-store` propagation, update this main-copy cursor and the `release/microsoft-store` worktree copy together.
Note: the cursor always points to the last propagated `main` state reflected in branch content, not to a docs-only cursor-update commit itself.
Alignment note: five runtime/UI commits after 1.0.46 and the 1.0.47 release preparation are reflected through the cursor above. Native-worker isolation, loading feedback, Chinese script selection, Soniox connection authentication, tray error feedback and setting fallbacks are propagated. The unsigned certificate-attachment gate is present in both branches. Main-only documentation, dev/Playwright tooling (342b37ed) and standalone frontend tests remain excluded. Store AVX2, updater, signing and existing test-support differences are preserved. Store Cargo.lock was regenerated from its own manifest and verified with locked offline metadata. Release notes and EN/RU What's New describe changes since 1.0.46. The user reported successful testing; no agent build/tests were run.
Sync rule: for this branch, source commits come from `main` only.
Propagation scope rule: for Microsoft Store Edition propagation, bring over the intended `main` commit set in order unless a commit is store-incompatible. Default exclusions are self-update/auto-update changes and AVX512-only changes; AVX2 is allowed.

## integration/cuda (frozen)

Frozen branch head: `ac2ee48a` — docs(sync): record 1.0.22 CUDA propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.

## integration/combined (frozen)

Frozen branch head: `10d35c4f` — docs(sync): record 1.0.22 combined propagation.
Keep its existing build documentation and release infrastructure intact, but do not propagate `main` updates into this branch unless the user explicitly unfreezes it.
