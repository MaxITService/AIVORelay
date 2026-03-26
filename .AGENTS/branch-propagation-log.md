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
| 2026-03-26 | `Microsoft-store` | `d0db58e4` | chore(bindings): regenerate after gpu selector sync | `0ea1294d` | chore(bindings): regenerate after gpu selector sync | includes 637f1e6a; clean cherry-pick |
| 2026-03-25 | `Microsoft-store` | `2dfc6ae3` | docs(sync): record v0.8.0 intake cursor | `46dabe77` | docs(sync): record v0.8.0 intake cursor | 4 commits; clean cherry-pick |
| 2026-03-24 | `Microsoft-store` | `869dea81` | chore(release): load release body from markdown | `f4202690` | chore(release): load release body from markdown | kept store-specific lead-in text |
| 2026-03-23 | `Microsoft-store` | `8c52c9f0` | chore: bump version to 1.0.6 | `96af69bd` | chore: bump version to 1.0.6 | includes e0b993b6; reused local 1.0.5 bump |
| 2026-03-22 | `Microsoft-store` | `65c2b65b` | fix(history): keep paginated history in sync | `726cb79a` | fix(history): keep paginated history in sync | includes 951a73e1; clean cherry-pick |
| 2026-03-22 | `Microsoft-store` | `9da5f76f` | feat(history): save recordings before transcription | `50457733` | feat(history): save recordings before transcription | conflict only in upstream sync log |
| 2026-03-21 | `Microsoft-store` | `5508e69a` | docs(sync): record AGENTS wording backfill for Microsoft-store | `c110904a` | sync(ms-store): align non-store-specific files with main | force overwrite path; preserved only Store-specific docs/config/workflow/updater/AVX2 files |
| 2026-03-21 | `Microsoft-store` | `9864ac9f` | agents | `0aed32ef` | agents | clean AGENTS docs backfill on top of Microsoft Store branch notes |
| 2026-03-21 | `Microsoft-store` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `a63069ae` | docs(agents): allow safe local verification commands | kept stricter Microsoft Store branch preface and newer safe-verification wording |
| 2026-03-21 | `Microsoft-store` | `463a0947` | Add per-microphone input boost and sync branch docs | `c521b2a6` | Add per-microphone input boost and sync branch docs | feature already present; kept richer branch implementation and resolved only absorbed tail |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
