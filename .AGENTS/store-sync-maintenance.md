Branch tags: #branch/release-microsoft-store

# Store Sync Maintenance

Procedure for propagating `main` into `release/microsoft-store`. The detailed general playbook lives on `main`; this file records the Store-specific boundary.

## Source And Cursor

- Use `main` as the only source.
- Read the source cursor from [[.AGENTS/branching-status|branching-status.md]], then verify it against Git history before applying anything.
- A cursor means every source commit through that point was reviewed, including commits intentionally excluded from Store.
- `git cherry` is supporting evidence only: adapted cherry-picks can have different patch IDs.

Useful read-only checks:

```powershell
git log --first-parent --oneline <cursor>..main
git cherry -v release/microsoft-store main
git reflog release/microsoft-store
```

## Propagation Order

1. Apply source commits in first-parent order, using batches while they remain conflict-free.
2. Preserve individual source commit messages for propagated runtime changes.
3. Resolve Store contract paths deliberately; never accept one side wholesale without reviewing the diff.
4. Regenerate and verify `src-tauri/Cargo.lock` after manifest propagation.
5. Audit documentation after runtime propagation.
6. Update the cursor and newest rolling-log row in both worktrees.

## Protected Or Manual-Review Paths

- `src-tauri/Cargo.lock`: regenerate locally; never cherry-pick.
- `src-tauri/tauri.conf.json`, `src-tauri/.cargo/**`, and Store CMake overrides: preserve Store packaging and AVX2 rules.
- `.github/workflows/**`: preserve Store tag/signing behavior; adapt only with explicit user approval.
- Updater commands, endpoints, artifacts, UI fallbacks, dependency-only bumps, and translations: exclude by default and review manually.
- AVX512/AMX distribution changes: exclude. AVX2-compatible shared runtime work is allowed.
- `AGENTS.md` and `.AGENTS/**`: keep branch-local except for the mirrored cursor and propagation row.
- Shared `.md` files: defer during runtime propagation. Sync CLI guides; manually review other user-facing documents.

## Completion Record

- Keep [[.AGENTS/branching-status|branching-status.md]] and [[.AGENTS/branch-propagation-log|branch-propagation-log.md]] in both `main` and Store.
- Store and `main` histories diverge because propagation uses adapted cherry-picks; do not use raw ahead/behind counts as the completion test.
- Record both SHA and commit message in chat and documentation.
