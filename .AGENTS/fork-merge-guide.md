# Upstream Sync & Merge Guide

This document defines the **safe intake workflow** for upstream commits in this fork.
Goal: keep selection logic strong, reduce breakage risk, and make decisions auditable.

## Upstream Tracking

Source of truth for recently integrated upstream commits:
- [[.AGENTS/upstream-sync-log|upstream-sync-log.md]]

Log rules:
- Keep newest entries first.
- Keep only last 10 entries.
- When adding the 11th entry, delete the oldest one.
- Each entry must include: upstream date; upstream SHA + message; merge date; local SHA + message; short issue note.

Current branch sync notes: [[branching-status]].

Useful commands:

```bash
git fetch upstream
# take <last_upstream_sha> from upstream-sync-log.md (newest row)
git log <last_upstream_sha>..upstream/main --oneline
git cherry -v main upstream/main
```

## 1) Commit Selection Logic (Keep This)

### Take
- Windows-relevant runtime fixes.
- Hotkey/input/shortcut fixes.
- STT/transcription/core audio pipeline fixes.
- Dependency/security updates used by active Windows code paths.
- Tray/UI fixes that affect Windows behavior.

### Optional
- Pure translations.
- Small UX improvements with limited risk.
- Changes with partial value but high conflict surface (apply selectively).

### Skip
- Release-only bumps/tags.
- Merge commits.
- Sponsor/PR template/issue template/document-only housekeeping.
- Linux/macOS-only runtime changes (unless they include shared critical fixes).

### Dependency Chain Rule
- If commit B depends on commit A, apply A first.
- If a newer commit supersedes older dependency bumps, prefer the newer one.

## 2) Report-First Rule (Before Any Cherry-Pick)

When user asks for upstream analysis, first write a **detailed HTML audit report** to:

`C:\Code\Released Software\AIVORelay\.AGENTS\.UNTRACKED\`

Recommended file naming:

`upstream-sync-audit-YYYY-MM-DD.html`

Required report sections:
1. Scope and audit date.
2. Summary counters: take / optional / skip.
3. Main table sorted by date (newest first).
4. Recommended application order (explicit sequence).
5. Already-applied mapping (upstream -> local hash), if known.
6. Conflict hotspots and mitigation notes.

Required columns in main table:
- Date
- SHA
- Subject
- Decision
- Priority
- Conflict risk
- Why it matters for this fork
- Expected conflict files (or `none`)

## 3) Execution Workflow (Only If User Approves)

Path selection rule:
- Fast path: direct cherry-pick is allowed when the commit is clearly relevant and expected to apply cleanly.
- Diff path (required): before applying, save a diff snapshot to `.AGENTS/.UNTRACKED/<sha>.diff.txt` when risk is non-trivial, confidence is low, or conflict is likely.
- If a cherry-pick fails/conflicts unexpectedly, immediately save the upstream diff file and continue from that snapshot.
- If conflicts are many/high-risk (for example: multiple files or fork hotspot files), run `git cherry-pick --abort` and switch to pure diff-path reverse-engineering. Do not use `git cherry-pick --continue` in that case.
1. Confirm working tree status and remember starting branch.
2. Switch to `main`.
3. Cherry-pick selected commits **one by one**.
4. If conflicts are small and safe, resolve and continue; if conflicts are many/high-risk, abort cherry-pick and apply via diff-path.
5. Record resulting `main` commit hashes.
6. If the user explicitly requests propagation, switch to `Microsoft-store`.
7. If requested, cherry-pick resulting `main` hashes in same logical order.
8. Return to original branch.
9. Provide concise status report:
   - `branch -> success/skipped/conflict`
   - upstream SHA -> resulting local SHA

## 4) Cargo.lock Policy

Never cherry-pick `src-tauri/Cargo.lock` from upstream directly.

If conflict occurs:

```bash
git checkout --ours src-tauri/Cargo.lock
git add src-tauri/Cargo.lock
```

Then continue cherry-pick. Lock refresh happens during user-driven build/check flow.

## 5) Conflict Hotspots (Fork-Specific)

### High-risk
- `src-tauri/src/settings.rs`
- `src-tauri/src/actions.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/shortcut.rs`

### Medium-risk
- `src-tauri/Cargo.toml`
- `src/hooks/useSettings.ts`
- `src/App.tsx`
- `src-tauri/resources/default_settings.json`

### Strategy
- Keep fork-only features intact (remote STT, connector, AI replace, screenshot flows, extra overlay states).
- Accept upstream improvements around unrelated areas.
- Merge carefully in shared files; do not drop fork-added commands/settings/bindings.

## 6) Intake Path And Optional Propagation

- Intake branch: `main`
- Propagation branch: `Microsoft-store` (only on explicit user request)
- `cuda-integration` is abandoned and excluded.

Never cherry-pick directly from `upstream` to `Microsoft-store`; propagate from `main` only when explicitly requested by the user.

## 7) Post-Sync Notes

After each successful sync, append a row to:
- [[.AGENTS/upstream-sync-log|upstream-sync-log.md]]

Do not store sync history as a single static cursor in this guide.


