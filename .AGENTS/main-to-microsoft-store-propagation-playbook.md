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

## Crash Recovery And Diagnostics

- Shared render recovery, frontend error logging, overlay boundaries, and their logging permissions are eligible runtime fixes.
- Keep the Store crash screen limited to recovery/log actions; do not add the emergency self-update checker, its imports, or its optional UI boundary.
- `927586a9` — Offer update checks on the whole-window crash screen — is an intentional exclusion. Later logging/overlay commits touch the same component: adapt their eligible hunks without restoring that updater. Keep the shared `SilentCrashBoundary` for overlays.
- Hidden frontend sourcemaps stored outside `dist` and uploaded as a separate CI artifact are eligible diagnostics. Preserve Store release tags, signing, packaging, and CPU settings when adapting the shared workflow.
- Keep `main`'s `.AGENTS/code-notes.md` branch-local. Review shared Help changes separately and include them only when the user has authorized that document's sync.
