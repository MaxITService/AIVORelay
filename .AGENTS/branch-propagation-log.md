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
| 2026-03-29 | `cuda-integration` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `f410d788` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `305c6878` | fix(settings): repair invalid portions on load | `4a983f07` | fix(settings): repair invalid portions on load | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `1c886375` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `9cf5c37a` | fix(ui): keep dropdown menus above settings panels | `0aec707f` | fix(ui): keep dropdown menus above settings panels | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `d0db58e4` | chore(bindings): regenerate after gpu selector sync | `65b99aba` | chore(bindings): regenerate after gpu selector sync | includes 637f1e6a; kept CUDA-specific transcribe-rs dependency path |
| 2026-03-25 | `cuda-integration` | `2dfc6ae3` | docs(sync): record v0.8.0 intake cursor | `11169944` | docs(sync): record v0.8.0 intake cursor | 4 commits; upstream-sync-log conflict only; kept CUDA-specific dependency path |
| 2026-03-24 | `cuda-integration` | `869dea81` | chore(release): load release body from markdown | `54bfd4c7` | chore(release): load release body from markdown | code-notes conflict only |
| 2026-03-23 | `cuda-integration` | `8c52c9f0` | chore: bump version to 1.0.6 | `ef3b9058` | chore: bump version to 1.0.6 | includes e0b993b6; reused local 1.0.5 bump |
| 2026-03-22 | `cuda-integration` | `65c2b65b` | fix(history): keep paginated history in sync | `2071d410` | fix(history): keep paginated history in sync | includes 951a73e1; clean cherry-pick |
| 2026-03-22 | `cuda-integration` | `9da5f76f` | feat(history): save recordings before transcription | `118a9e86` | feat(history): save recordings before transcription | upstream-sync-log conflict resolved with ours |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
