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
| 2026-09-06 | `release/microsoft-store` | `0ca930c0` | Revert "feat(text-replacement): add quick literal replacement dialog" | `d47dd52b` | Revert "feat(text-replacement): add quick literal replacement dialog" | Quick Replacement removed from runtime, Help, search, and release notes; later features preserved |
| 2026-09-06 | `release/microsoft-store` | `7545d18a` | feat(tray): add opt-in speech-only mode command | `dcac80bc` | feat(tray): add opt-in speech-only mode command | optional certificate guide and tray command propagated manually; Store release text adapted |
| 2026-09-06 | `release/microsoft-store` | `9da54b31` | chore: bump version to 1.0.36 | `6f649239` | chore: bump version to 1.0.36 | prompt reuse and Gemini Live availability fix propagated manually; Store release text adapted; lock version regenerated locally |
| 2026-09-06 | `release/microsoft-store` | `8271ee6e` | fix(settings): improve search across localized menus | `b88ef92e` | fix(settings): improve search across localized menus | runtime/UI commit propagated cleanly; main-only sync documentation excluded |
| 2026-09-06 | `release/microsoft-store` | `2a23c076` | feat(settings): allow longer paste delays | `7eaf6a5c` | feat(settings): allow longer paste delays | one-line upstream UI adaptation; upstream-only audit documentation excluded |
| 2026-09-06 | `release/microsoft-store` | `f07fe24d` | fix(audio): prevent recording stop races and hangs | `0315472e` | fix(audio): prevent recording stop races and hangs | 16 runtime/UI commits propagated manually; main-only docs and Cargo.lock excluded; one startup conflict resolved for Store |
| 2026-09-02 | `release/microsoft-store` | `aaedfecd` | test: use Developer PowerShell for local Rust setup | `ba6021ad` | ci(store): audit x64 transcribe runtime packaging | remaining main runtime propagated; updater and AVX512 excluded; lock regenerated; applicable docs and x64 packaging checks updated |
| 2026-07-24 | `release/microsoft-store` | `3d772235` | chore: bump version to 1.0.26 | `777eedd2` | chore: bump version to 1.0.26 | 2 runtime fixes propagated; main-only docs and updater binding excluded; Store notes updated; lock version updated locally |
| 2026-07-22 | `release/microsoft-store` | `5b22f470` | chore: bump version to 1.0.25 | `c2d7c994` | chore: bump version to 1.0.25 | 15 runtime updates propagated; main-only docs excluded; Store notes updated; lock regenerated locally |
| 2026-07-15 | `release/microsoft-store` | `86242355` | chore: bump version to 1.0.24 | `482f6bcc` | chore: bump version to 1.0.24 | 11 runtime commits propagated; main-only docs excluded; Store notes updated; lock resolved locally |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
