# Branch Propagation Log

Small rolling log of `main` commits propagated into non-`main` release branches.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per branch propagation event.
- On new entry #11, remove the oldest row.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.
- Keep issue notes very short.

| Propagation Date | Target Branch | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-19 | `cuda-integration` | `30895116` | chore(bindings): regenerate tauri bindings | `a14f2913` | chore(bindings): regenerate tauri bindings | clean cherry-pick |
| 2026-03-19 | `Microsoft-store` | `30895116` | chore(bindings): regenerate tauri bindings | `eac6b050` | chore(bindings): regenerate tauri bindings | clean cherry-pick |
| 2026-03-19 | `cuda-integration` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `81c63888` | docs(agents): allow safe local verification commands | backfill cherry-pick |
| 2026-03-19 | `Microsoft-store` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `8c37a2e6` | docs(agents): allow safe local verification commands | backfill cherry-pick; `AGENTS.md` conflict |
| 2026-03-19 | `cuda-integration` | `1a13f626` | fix(ui): stabilize microphone boost slider | `dc28b67f` | fix(ui): stabilize microphone boost slider | 2 picks; manual merge in `recorder.rs`, `settings.rs`, `lib.rs` |
| 2026-03-19 | `Microsoft-store` | `1a13f626` | fix(ui): stabilize microphone boost slider | `b3b0b0f6` | fix(ui): stabilize microphone boost slider | 2 picks; manual merge in `recorder.rs`, `settings.rs`, `lib.rs` |
| 2026-03-15 | `cuda-integration` | `ca08fe72` | feat(settings): repair invalid settings and bump version to 1.0.3 | `fc78b7b6` | feat(settings): repair invalid settings and bump version to 1.0.3 | clean cherry-pick |
| 2026-03-15 | `Microsoft-store` | `ca08fe72` | feat(settings): repair invalid settings and bump version to 1.0.3 | `c0fbd1c3` | feat(settings): repair invalid settings and bump version to 1.0.3 | clean cherry-pick |
| 2026-03-14 | `cuda-integration` | `7f99cb5` | Notify on model loading failures | `54aae0d` | Notify on model loading failures | HandyKeys `Cargo.toml` conflict; kept CUDA `whisper-rs` + ours lock |
| 2026-03-12 | `Microsoft-store` | `0efb3f7` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | `6f6e4e8` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
