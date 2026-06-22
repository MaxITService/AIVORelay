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
| 2026-06-22 | `integration/combined` | `1e1f7379` | chore: bump version to 1.0.21 | `501d2108` | chore: bump version to 1.0.21 | clean propagation |
| 2026-06-18 | `integration/combined` | `aed4b537` | chore: bump version to 1.0.20 | `c6b21c92` | chore: bump version to 1.0.20 | clean propagation |
| 2026-06-03 | `integration/combined` | `9c9bc15b` | fix(installer): use executable icon for app shortcuts | `bf18a8c0` | chore: bump version to 1.0.19 | installer icon already matched |
| 2026-06-01 | `integration/combined` | `8ef91425` | chore: bump version to 1.0.18 | `43cff58e` | chore: bump version to 1.0.18 | resolved tray, history layout, branch docs/deps |
| 2026-05-11 | `integration/combined` | `bf6cae7d` | chore: bump version to 1.0.17 | `304bdf24` | chore: bump version to 1.0.17 | resolved settings import conflict; combined release notes excluded |
| 2026-05-10 | `integration/combined` | `a2e3a115` | fix(settings): unify API key controls | `644f7ed7` | fix(settings): unify API key controls | clean cherry-pick |
| 2026-05-03 | `integration/combined` | `b476149b` | chore: bump version to 1.0.15 | `ecbfe423` | chore: bump version to 1.0.15 | resolved tray import, overlay, bindings/settings conflicts |
| 2026-04-25 | `integration/combined` | `4b6adfa5` | fix(overlay): keep error layout onscreen | `d2f38a73` | fix(overlay): keep error layout onscreen | resolved old geometry helper conflict |
| 2026-04-18 | `integration/combined` | `f36a1cdc` | chore(bindings): commit pending generated update | `0b555fcc` | chore(bindings): commit pending generated update | clean cherry-pick |
| 2026-04-18 | `integration/cuda` | `f36a1cdc` | chore(bindings): commit pending generated update | `143aa0d0` | chore(bindings): commit pending generated update | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
