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
| 2026-03-21 | `Microsoft-store` | `653e69ea` | Fix accelerator command state access | `7bb22aaa` | Fix accelerator command state access | resolved command state type onto existing `Arc<TranscriptionManager>` wiring |
| 2026-03-21 | `Microsoft-store` | `20b116dd` | Document recording buffer intake | `3d0515f7` | Document recording buffer intake | upstream log already newer; preserved current rows and applied recording-buffer docs notes |
| 2026-03-21 | `Microsoft-store` | `a53b07a1` | Document GigaAM upstream intake | `62158743` | Document GigaAM upstream intake | upstream log already newer; preserved current rows and applied GigaAM docs notes |
| 2026-03-21 | `Microsoft-store` | `ae4cab39` | Document March upstream intake | `50c7d9f9` | Document March upstream intake | upstream log already newer; applied code-notes expansion and preserved current rows |
| 2026-03-21 | `Microsoft-store` | `fefc6bf0` | Document code quality intake | `ff934e5f` | Document code quality intake | upstream log already newer; kept current rows and applied docs note |
| 2026-03-21 | `Microsoft-store` | `5dc33a2d` | Consolidate PR code quality checks | `20f2eb8e` | Consolidate PR code quality checks | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `8fe4b1fd` | Promo | `62ff3a69` | Promo | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `96ebb0e7` | Fix clamshell test import warning | `2a644fda` | Fix clamshell test import warning | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `25a03b17` | Port GigaAM v3 directory migration | `96041be0` | Port GigaAM v3 directory migration | migration already present; resolved remaining SHA256/resource tail |
| 2026-03-21 | `Microsoft-store` | `70163254` | Improve idle model unload behavior | `382ef7b6` | Improve idle model unload behavior | clean cherry-pick during post-1.0.3 backfill |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
