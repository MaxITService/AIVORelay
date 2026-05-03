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
| 2026-05-03 | `integration/combined` | `b476149b` | chore: bump version to 1.0.15 | `ecbfe423` | chore: bump version to 1.0.15 | resolved tray import, overlay, bindings/settings conflicts |
| 2026-05-03 | `integration/cuda` | `b476149b` | chore: bump version to 1.0.15 | `69bc618b` | chore: bump version to 1.0.15 | resolved overlay and bindings/settings conflicts |
| 2026-05-03 | `release/microsoft-store` | `b476149b` | chore: bump version to 1.0.15 | `1a303b50` | chore: bump version to 1.0.15 | resolved bindings conflict; main release notes excluded |
| 2026-04-25 | `integration/combined` | `4b6adfa5` | fix(overlay): keep error layout onscreen | `d2f38a73` | fix(overlay): keep error layout onscreen | resolved old geometry helper conflict |
| 2026-04-25 | `integration/cuda` | `4b6adfa5` | fix(overlay): keep error layout onscreen | `ff2e581c` | fix(overlay): keep error layout onscreen | resolved old geometry helper conflict |
| 2026-04-25 | `release/microsoft-store` | `4b6adfa5` | fix(overlay): keep error layout onscreen | `f8131ee0` | fix(overlay): keep error layout onscreen | clean cherry-pick |
| 2026-04-25 | `release/microsoft-store` | `1036ed36` | chore: bump version to 1.0.14 | `222b079f` | chore: bump version to 1.0.14 | store release notes only |
| 2026-04-25 | `release/microsoft-store` | `70200fd6` | feat(history): allow editing dictation stats | `77eb7680` | feat(history): allow editing dictation stats | clean cherry-pick |
| 2026-04-21 | `release/microsoft-store` | `05e365c9` | fix(transcription): ensure local model loads before transcription | `1f058a22` | fix(transcription): ensure local model loads before transcription | clean cherry-pick |
| 2026-04-21 | `release/microsoft-store` | `72663503` | Counter for words and characters. | `30d789a5` | Counter for words and characters. | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
