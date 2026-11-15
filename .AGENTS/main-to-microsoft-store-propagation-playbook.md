# Main To release/microsoft-store Propagation Playbook
Branch tags: #branch/main #branch/release-microsoft-store

This playbook contains specific rules for `main -> release/microsoft-store` propagation.
For the general workflow, start point rules, and documentation handling, you MUST follow:
- [[.AGENTS/general-branch-propagation-playbook|general-branch-propagation-playbook.md]]

## Propagation Scope

Target branch: `release/microsoft-store`

**Default exclusions for Microsoft Store Edition:**
- documentation changes (General Rule applied)
- self-update / auto-update changes
- AVX512-only changes

**Allowed by default:**
- AVX2-targeted changes
- normal runtime fixes from `main`
- UI fixes from `main`

Store release contract: [[../AIVORelay-store/.AGENTS/store-branch-notes#Distribution Differences|Microsoft Store branch contract]].

## Shared Application Source

From 1.0.54, both branches use identical application source under `src/`, `src-tauri/src/`, and `src-tauri/build.rs`. The branch-local `src-tauri/app-edition` is `standard` on main and `store` on Store. It is compiled into the binary and must never be overwritten during propagation.

- Shared updater code is eligible. `distribution.rs` enforces the Store restriction for MSI, ZIP and MSIX; the crash screen never mounts the updater while its backend lock is unknown or enabled.
- Keep Store updater endpoints empty and updater artifacts disabled. The build script validates these with the Store marker.
- Keep AVX2, signing, dependency and packaging configuration branch-local. Never copy Cargo.lock from main.
- Common source and committed generated bindings should stay identical. Review source differences and bring fixes to main first; avoid new branch-specific runtime edits.

## Crash Recovery And Diagnostics

- Shared render recovery, frontend error logging, overlay boundaries, and their logging permissions are eligible runtime fixes.
- Keep the Store crash screen limited to recovery/log actions through the shared edition guard; updater imports may be shared, but the updater must not mount for Store.
- Before 1.0.54, `927586a9` was excluded by removing updater code. The common edition guard now preserves that behavior without divergent source. Keep the shared `SilentCrashBoundary` for overlays.
- Hidden frontend sourcemaps stored outside `dist` and uploaded as a separate CI artifact are eligible diagnostics. Preserve Store release tags, signing, packaging, and CPU settings when adapting the shared workflow.
- Keep `main`'s `.AGENTS/code-notes.md` branch-local. Review shared Help changes separately and include them only when the user has authorized that document's sync.
