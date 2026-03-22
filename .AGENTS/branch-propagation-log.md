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
| 2026-03-22 | `cuda-integration` | `9da5f76f` | feat(history): save recordings before transcription | `118a9e86` | feat(history): save recordings before transcription | upstream-sync-log conflict resolved with ours |
| 2026-03-22 | `cuda-integration` | `5bcc9c6b` | feat(audio): add recording overlay appearance customization | `self` | sync(cuda): hard align branch to main head | hard overwrite; preserved CUDA wiring/docs |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
