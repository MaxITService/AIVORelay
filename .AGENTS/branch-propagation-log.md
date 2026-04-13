# Branch Propagation Log
Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

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
| 2026-04-14 | `integration/combined` | `f2874155` | feat(history): add delete all action | `d9375635` | feat(history): add delete all action | clean cherry-pick |
| 2026-04-14 | `integration/cuda` | `f2874155` | feat(history): add delete all action | `bb3bc334` | feat(history): add delete all action | clean cherry-pick |
| 2026-04-14 | `release/microsoft-store` | `f2874155` | feat(history): add delete all action | `e9fc97f8` | feat(history): add delete all action | clean cherry-pick |
| 2026-04-14 | `integration/combined` | `827ae3da` | fix(settings): separate history controls from entries | `b6de045c` | fix(settings): separate history controls from entries | clean cherry-pick |
| 2026-04-14 | `integration/cuda` | `827ae3da` | fix(settings): separate history controls from entries | `fb2f7123` | fix(settings): separate history controls from entries | clean cherry-pick |
| 2026-04-14 | `release/microsoft-store` | `827ae3da` | fix(settings): separate history controls from entries | `990b7c1b` | fix(settings): separate history controls from entries | clean cherry-pick |
| 2026-04-13 | `integration/combined` | `8268eff1` | fix(settings): show history controls in history panel | `b6f8ddc2` | fix(settings): show history controls in history panel | clean cherry-pick |
| 2026-04-13 | `integration/cuda` | `8268eff1` | fix(settings): show history controls in history panel | `b1043368` | fix(settings): show history controls in history panel | clean cherry-pick |
| 2026-04-13 | `release/microsoft-store` | `8268eff1` | fix(settings): show history controls in history panel | `5ef8b775` | fix(settings): show history controls in history panel | clean cherry-pick |
| 2026-04-13 | `integration/combined` | `b5d14fef` | chore(bindings): refresh pause media recording setting | `02242f89` | chore(bindings): refresh pause media recording setting | clean cherry-pick |
| 2026-04-13 | `integration/cuda` | `b5d14fef` | chore(bindings): refresh pause media recording setting | `9ffffa01` | chore(bindings): refresh pause media recording setting | clean cherry-pick |
| 2026-04-13 | `release/microsoft-store` | `b5d14fef` | chore(bindings): refresh pause media recording setting | `3cbf6cfa` | chore(bindings): refresh pause media recording setting | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
