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
| 2026-03-22 | `Microsoft-store` | `65c2b65b` | fix(history): keep paginated history in sync | `726cb79a` | fix(history): keep paginated history in sync | includes 951a73e1; clean cherry-pick |
| 2026-03-22 | `Microsoft-store` | `9da5f76f` | feat(history): save recordings before transcription | `50457733` | feat(history): save recordings before transcription | conflict only in upstream sync log |
| 2026-03-21 | `Microsoft-store` | `5508e69a` | docs(sync): record AGENTS wording backfill for Microsoft-store | `c110904a` | sync(ms-store): align non-store-specific files with main | force overwrite path; preserved only Store-specific docs/config/workflow/updater/AVX2 files |
| 2026-03-21 | `Microsoft-store` | `9864ac9f` | agents | `0aed32ef` | agents | clean AGENTS docs backfill on top of Microsoft Store branch notes |
| 2026-03-21 | `Microsoft-store` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `a63069ae` | docs(agents): allow safe local verification commands | kept stricter Microsoft Store branch preface and newer safe-verification wording |
| 2026-03-21 | `Microsoft-store` | `463a0947` | Add per-microphone input boost and sync branch docs | `c521b2a6` | Add per-microphone input boost and sync branch docs | feature already present; kept richer branch implementation and resolved only absorbed tail |
| 2026-03-21 | `Microsoft-store` | `48988a2f` | Update bindings for accelerator settings | `b88ebe50` | Update bindings for accelerator settings | clean bindings backfill after accelerator help sync |
| 2026-03-21 | `Microsoft-store` | `90d868ce` | Sync acceleration i18n and add help | `28aa51db` | Sync acceleration i18n and add help | merged help/i18n onto existing accelerator layer and preserved branch-local lazy mic text |
| 2026-03-21 | `Microsoft-store` | `653e69ea` | Fix accelerator command state access | `7bb22aaa` | Fix accelerator command state access | resolved command state type onto existing `Arc<TranscriptionManager>` wiring |
| 2026-03-21 | `Microsoft-store` | `20b116dd` | Document recording buffer intake | `3d0515f7` | Document recording buffer intake | upstream log already newer; preserved current rows and applied recording-buffer docs notes |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
