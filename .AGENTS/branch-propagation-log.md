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
| 2026-03-26 | `cuda-integration` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `1c886375` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |
| 2026-03-26 | `Microsoft-store` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `a6974073` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `9cf5c37a` | fix(ui): keep dropdown menus above settings panels | `0aec707f` | fix(ui): keep dropdown menus above settings panels | clean cherry-pick |
| 2026-03-26 | `Microsoft-store` | `9cf5c37a` | fix(ui): keep dropdown menus above settings panels | `88c25f1a` | fix(ui): keep dropdown menus above settings panels | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `d0db58e4` | chore(bindings): regenerate after gpu selector sync | `65b99aba` | chore(bindings): regenerate after gpu selector sync | includes 637f1e6a; kept CUDA-specific transcribe-rs dependency path |
| 2026-03-26 | `Microsoft-store` | `d0db58e4` | chore(bindings): regenerate after gpu selector sync | `0ea1294d` | chore(bindings): regenerate after gpu selector sync | includes 637f1e6a; clean cherry-pick |
| 2026-03-25 | `cuda-integration` | `2dfc6ae3` | docs(sync): record v0.8.0 intake cursor | `11169944` | docs(sync): record v0.8.0 intake cursor | 4 commits; upstream-sync-log conflict only; kept CUDA-specific dependency path |
| 2026-03-25 | `Microsoft-store` | `2dfc6ae3` | docs(sync): record v0.8.0 intake cursor | `46dabe77` | docs(sync): record v0.8.0 intake cursor | 4 commits; clean cherry-pick |
| 2026-03-24 | `Microsoft-store` | `869dea81` | chore(release): load release body from markdown | `f4202690` | chore(release): load release body from markdown | kept store-specific lead-in text |
| 2026-03-24 | `cuda-integration` | `869dea81` | chore(release): load release body from markdown | `54bfd4c7` | chore(release): load release body from markdown | code-notes conflict only |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
