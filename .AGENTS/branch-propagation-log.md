# Branch Propagation Log

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
| 2026-03-21 | `cuda-integration` | `ffb30103` | docs(sync): advance upstream intake log to 58cda3f3 | `a125e2dc` | docs(sync): advance upstream intake log to 58cda3f3 | upstream log conflict resolved by taking main copy |
| 2026-03-21 | `Microsoft-store` | `ffb30103` | docs(sync): advance upstream intake log to 58cda3f3 | `1346f4c6` | docs(sync): advance upstream intake log to 58cda3f3 | equivalent apply on dirty worktree; vendor patch was already staged |
| 2026-03-21 | `cuda-integration` | `e7ca2c90` | refactor(transcription): dedupe model load failure events | `d7387e5c` | fix(cuda): trim overlay merge fallout after audio intake | 6 picks; 2 CUDA fixes; release build ok |
| 2026-03-21 | `Microsoft-store` | `e7ca2c90` | refactor(transcription): dedupe model load failure events | `7635ad15` | refactor(transcription): dedupe model load failure events | 6 picks; manual merges; old `whisper-rs-sys` block |
| 2026-03-19 | `cuda-integration` | `c900c3fa` | chore(release): bump version to 1.0.4 | `5ff5e9df` | chore(release): bump version to 1.0.4 | clean cherry-pick |
| 2026-03-19 | `Microsoft-store` | `c900c3fa` | chore(release): bump version to 1.0.4 | `77745b15` | chore(release): bump version to 1.0.4 | clean cherry-pick |
| 2026-03-19 | `cuda-integration` | `30895116` | chore(bindings): regenerate tauri bindings | `a14f2913` | chore(bindings): regenerate tauri bindings | clean cherry-pick |
| 2026-03-19 | `Microsoft-store` | `30895116` | chore(bindings): regenerate tauri bindings | `eac6b050` | chore(bindings): regenerate tauri bindings | clean cherry-pick |
| 2026-03-19 | `cuda-integration` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `81c63888` | docs(agents): allow safe local verification commands | backfill cherry-pick |
| 2026-03-19 | `Microsoft-store` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `8c37a2e6` | docs(agents): allow safe local verification commands | backfill cherry-pick; `AGENTS.md` conflict |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
