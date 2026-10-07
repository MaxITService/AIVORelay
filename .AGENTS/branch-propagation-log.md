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
| 2026-10-08 | `release/microsoft-store` | `c5df5ef5` | chore: bump version to 1.0.46 | `94e6b955` | chore: bump version to 1.0.46 | 228 ordered main commits; main-only docs/certificate tooling, updater search and standalone frontend tests excluded; Store config/AVX2/dependencies preserved; lock app version updated independently; user confirmed build verification; no agent build/tests |
| 2026-10-02 | `release/microsoft-store` | `2a495378` | chore: bump version to 1.0.45 | `7818782c` | chore: bump version to 1.0.45 | Overlay presets, icon frames and 3D styles propagated; main-only docs and standalone frontend tests excluded; 1.0.44 highlights retained in 1.0.45; Store lock package version updated independently; no local build/tests |
| 2026-10-01 | `release/microsoft-store` | `d4b43e77` | feat: refine overlay appearance and local TTS consent | `b5eff6fd` | feat: refine overlay appearance and local TTS consent | 3 runtime commits propagated today; Soniox v2, startup/text fixes, overlay and TTS consent; tests/test-support refactors, testing docs and promo excluded; reviewed test-only 8498faaa; Store lock metadata verified; no build/tests |
| 2026-09-25 | `release/microsoft-store` | `1fae3dc9` | chore: bump version to 1.0.43 | `d36e0669` | chore: bump version to 1.0.43 | 4 runtime/UI commits propagated; main-only upstream log and release body excluded; Store notes adapted; lock version updated |
| 2026-09-19 | `release/microsoft-store` | `d2021f7b` | chore: bump version to 1.0.42 | `2e512d12` | chore: bump version to 1.0.42 | 8 runtime/UI commits propagated; main-only `.AGENTS/code-notes.md` excluded from 2 commits; lib.rs window-builder conflict resolved to main's create_main_window; Store notes adapted; lock version refreshed |
| 2026-09-19 | `release/microsoft-store` | `d4b13e3a` | chore: bump version to 1.0.41 | `64a43859` | chore: bump version to 1.0.41 | 10 runtime/UI commits propagated cleanly; main-only docs (CLAUDE.md, .AGENTS notes) excluded; Store notes adapted; lock version refreshed |
| 2026-09-17 | `release/microsoft-store` | `a2d79f3e` | chore: bump version to 1.0.40 | `f45b68cd` | chore: bump version to 1.0.40 | 3 Gemini runtime/UI commits propagated cleanly; Store notes adapted; lock version refreshed |
| 2026-09-16 | `release/microsoft-store` | `b1b0a285` | chore: bump version to 1.0.39 | `2c38add8` | chore: bump version to 1.0.39 | overlay UI-thread hotfix propagated cleanly; Store notes adapted; lock regenerated |
| 2026-09-16 | `release/microsoft-store` | `2aaf46d4` | chore: bump version to 1.0.38 | `583e9146` | chore: bump version to 1.0.38 | 77 runtime/UI/test and release commits propagated; main-only docs, updater, and intermediate 1.0.37 bump excluded; Store notes adapted; lock regenerated |
| 2026-09-06 | `release/microsoft-store` | `d18bfba8` | fix(copy): generalize realtime audio warning | `9da95e55` | fix(copy): generalize realtime audio warning | copy-only change propagated cleanly |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
