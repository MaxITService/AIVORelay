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
| 2026-03-21 | `Microsoft-store` | `fefc6bf0` | Document code quality intake | `ff934e5f` | Document code quality intake | upstream log already newer; kept current rows and applied docs note |
| 2026-03-21 | `Microsoft-store` | `5dc33a2d` | Consolidate PR code quality checks | `20f2eb8e` | Consolidate PR code quality checks | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `8fe4b1fd` | Promo | `62ff3a69` | Promo | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `96ebb0e7` | Fix clamshell test import warning | `2a644fda` | Fix clamshell test import warning | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `25a03b17` | Port GigaAM v3 directory migration | `96041be0` | Port GigaAM v3 directory migration | migration already present; resolved remaining SHA256/resource tail |
| 2026-03-21 | `Microsoft-store` | `70163254` | Improve idle model unload behavior | `382ef7b6` | Improve idle model unload behavior | clean cherry-pick during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `3346d7d8` | ensure samples don't get dropped (#1043) | `d2ca9803` | ensure samples don't get dropped (#1043) | manual merge in `recorder.rs` during post-1.0.3 backfill |
| 2026-03-21 | `Microsoft-store` | `5dd380a0` | docs: add branch sync status tracking file | `53cf016f` | docs: add branch sync status tracking file | historical docs backfill |
| 2026-03-21 | `cuda-integration` | `ffb30103` | docs(sync): advance upstream intake log to 58cda3f3 | `a125e2dc` | docs(sync): advance upstream intake log to 58cda3f3 | upstream log conflict resolved by taking main copy |
| 2026-03-21 | `Microsoft-store` | `ffb30103` | docs(sync): advance upstream intake log to 58cda3f3 | `1346f4c6` | docs(sync): advance upstream intake log to 58cda3f3 | equivalent apply on dirty worktree; vendor patch was already staged |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
