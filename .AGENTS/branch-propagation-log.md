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
| 2026-03-21 | `Microsoft-store` | `f5bcf1bf` | docs(agents): allow safe local verification commands | `a63069ae` | docs(agents): allow safe local verification commands | kept stricter Microsoft Store branch preface and newer safe-verification wording |
| 2026-03-21 | `Microsoft-store` | `463a0947` | Add per-microphone input boost and sync branch docs | `c521b2a6` | Add per-microphone input boost and sync branch docs | feature already present; kept richer branch implementation and resolved only absorbed tail |
| 2026-03-21 | `Microsoft-store` | `48988a2f` | Update bindings for accelerator settings | `b88ebe50` | Update bindings for accelerator settings | clean bindings backfill after accelerator help sync |
| 2026-03-21 | `Microsoft-store` | `90d868ce` | Sync acceleration i18n and add help | `28aa51db` | Sync acceleration i18n and add help | merged help/i18n onto existing accelerator layer and preserved branch-local lazy mic text |
| 2026-03-21 | `Microsoft-store` | `653e69ea` | Fix accelerator command state access | `7bb22aaa` | Fix accelerator command state access | resolved command state type onto existing `Arc<TranscriptionManager>` wiring |
| 2026-03-21 | `Microsoft-store` | `20b116dd` | Document recording buffer intake | `3d0515f7` | Document recording buffer intake | upstream log already newer; preserved current rows and applied recording-buffer docs notes |
| 2026-03-21 | `Microsoft-store` | `a53b07a1` | Document GigaAM upstream intake | `62158743` | Document GigaAM upstream intake | upstream log already newer; preserved current rows and applied GigaAM docs notes |
| 2026-03-21 | `Microsoft-store` | `ae4cab39` | Document March upstream intake | `50c7d9f9` | Document March upstream intake | upstream log already newer; applied code-notes expansion and preserved current rows |
| 2026-03-21 | `Microsoft-store` | `fefc6bf0` | Document code quality intake | `ff934e5f` | Document code quality intake | upstream log already newer; kept current rows and applied docs note |
| 2026-03-21 | `Microsoft-store` | `5dc33a2d` | Consolidate PR code quality checks | `20f2eb8e` | Consolidate PR code quality checks | clean cherry-pick during post-1.0.3 backfill |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
