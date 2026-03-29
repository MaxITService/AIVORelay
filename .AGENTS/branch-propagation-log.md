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
| 2026-03-29 | `Microsoft-store` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `86e2dd8b` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `Microsoft-store` | `305c6878` | fix(settings): repair invalid portions on load | `749ed4b4` | fix(settings): repair invalid portions on load | clean cherry-pick |
| 2026-03-26 | `Microsoft-store` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `a6974073` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |
| 2026-03-26 | `Microsoft-store` | `9cf5c37a` | fix(ui): keep dropdown menus above settings panels | `88c25f1a` | fix(ui): keep dropdown menus above settings panels | clean cherry-pick |
| 2026-03-26 | `Microsoft-store` | `d0db58e4` | chore(bindings): regenerate after gpu selector sync | `0ea1294d` | chore(bindings): regenerate after gpu selector sync | includes 637f1e6a; clean cherry-pick |
| 2026-03-25 | `Microsoft-store` | `2dfc6ae3` | docs(sync): record v0.8.0 intake cursor | `46dabe77` | docs(sync): record v0.8.0 intake cursor | 4 commits; clean cherry-pick |
| 2026-03-24 | `Microsoft-store` | `869dea81` | chore(release): load release body from markdown | `f4202690` | chore(release): load release body from markdown | kept store-specific lead-in text |
| 2026-03-23 | `Microsoft-store` | `8c52c9f0` | chore: bump version to 1.0.6 | `96af69bd` | chore: bump version to 1.0.6 | includes e0b993b6; reused local 1.0.5 bump |
| 2026-03-22 | `Microsoft-store` | `65c2b65b` | fix(history): keep paginated history in sync | `726cb79a` | fix(history): keep paginated history in sync | includes 951a73e1; clean cherry-pick |
| 2026-03-22 | `Microsoft-store` | `9da5f76f` | feat(history): save recordings before transcription | `50457733` | feat(history): save recordings before transcription | conflict only in upstream sync log |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
