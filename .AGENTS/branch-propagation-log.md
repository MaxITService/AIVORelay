# Branch Propagation Log
Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

Small rolling log of `main` commits propagated into non-`main` release branches.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per branch propagation event.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.

| Propagation Date | Target Branch | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-09-19 | `release/microsoft-store` | `d2021f7b` | chore: bump version to 1.0.42 | `2e512d12` | chore: bump version to 1.0.42 | 8 runtime/UI commits propagated; main-only `.AGENTS/code-notes.md` excluded from 2 commits; lib.rs window-builder conflict resolved to main's create_main_window; Store notes adapted; lock version refreshed |
| 2026-09-19 | `release/microsoft-store` | `d4b13e3a` | chore: bump version to 1.0.41 | `64a43859` | chore: bump version to 1.0.41 | 10 runtime/UI commits propagated cleanly; main-only docs (CLAUDE.md, .AGENTS notes) excluded; Store notes adapted; lock version refreshed |
| 2026-09-17 | `release/microsoft-store` | `a2d79f3e` | chore: bump version to 1.0.40 | `f45b68cd` | chore: bump version to 1.0.40 | 3 Gemini runtime/UI commits propagated cleanly; Store notes adapted; lock version refreshed |
| 2026-09-16 | `release/microsoft-store` | `b1b0a285` | chore: bump version to 1.0.39 | `2c38add8` | chore: bump version to 1.0.39 | overlay UI-thread hotfix propagated cleanly; Store notes adapted; lock regenerated |
| 2026-09-16 | `release/microsoft-store` | `2aaf46d4` | chore: bump version to 1.0.38 | `583e9146` | chore: bump version to 1.0.38 | 77 runtime/UI/test and release commits propagated; main-only docs, updater, and intermediate 1.0.37 bump excluded; Store notes adapted; lock regenerated |
| 2026-09-06 | `release/microsoft-store` | `d18bfba8` | fix(copy): generalize realtime audio warning | `9da95e55` | fix(copy): generalize realtime audio warning | copy-only change propagated cleanly |
| 2026-09-06 | `release/microsoft-store` | `0ca930c0` | Revert "feat(text-replacement): add quick literal replacement dialog" | `d47dd52b` | Revert "feat(text-replacement): add quick literal replacement dialog" | Quick Replacement removed from runtime, Help, search, and release notes; later features preserved |
| 2026-09-06 | `release/microsoft-store` | `7545d18a` | feat(tray): add opt-in speech-only mode command | `dcac80bc` | feat(tray): add opt-in speech-only mode command | optional certificate guide and tray command propagated manually; Store release text adapted |
| 2026-09-06 | `release/microsoft-store` | `9da54b31` | chore: bump version to 1.0.36 | `6f649239` | chore: bump version to 1.0.36 | prompt reuse and Gemini Live availability fix propagated manually; Store release text adapted; lock version regenerated locally |
| 2026-09-06 | `release/microsoft-store` | `8271ee6e` | fix(settings): improve search across localized menus | `b88ef92e` | fix(settings): improve search across localized menus | runtime/UI commit propagated cleanly; main-only sync documentation excluded |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
