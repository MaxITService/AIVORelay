# Branch Propagation Log

Small rolling log of `main` commits propagated into non-`main` release branches.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per branch propagation event.
- On new entry #11, remove the oldest row.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.
- Keep issue notes very short.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.

| Propagation Date | Target Branch | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-15 | `cuda-integration` | `ca08fe72` | feat(settings): repair invalid settings and bump version to 1.0.3 | `fc78b7b6` | feat(settings): repair invalid settings and bump version to 1.0.3 | clean cherry-pick |
| 2026-03-15 | `Microsoft-store` | `ca08fe72` | feat(settings): repair invalid settings and bump version to 1.0.3 | `c0fbd1c3` | feat(settings): repair invalid settings and bump version to 1.0.3 | clean cherry-pick |
| 2026-03-14 | `cuda-integration` | `7f99cb5` | Notify on model loading failures | `54aae0d` | Notify on model loading failures | HandyKeys `Cargo.toml` conflict; kept CUDA `whisper-rs` + ours lock |
| 2026-03-12 | `Microsoft-store` | `0efb3f7` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | `6f6e4e8` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | clean cherry-pick |
| 2026-03-12 | `cuda-integration` | `0efb3f7` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | `41c6348` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | clean cherry-pick |
| 2026-03-11 | `cuda-integration` | `6029d1e` | feat: show file transcription benchmark time | `a918c1b` | feat: show file transcription benchmark time | clean cherry-pick |
| 2026-03-11 | `Microsoft-store` | `5fc34cd` | chore: bump version to 1.0.1 | `ba38285` | chore: bump version to 1.0.1 | store release bump |
| 2026-03-11 | `Microsoft-store` | `ad387df` | fix(ai-replace): restore custom provider setup | `e133f59` | fix(ai-replace): restore custom provider setup | clean cherry-pick |
| 2026-03-11 | `Microsoft-store` | `46566c1` | fix(voice-command): restore custom LLM endpoint config | `129ade1` | fix(voice-command): restore custom LLM endpoint config | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
