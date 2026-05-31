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
| 2026-06-01 | `integration/combined` | `8ef91425` | chore: bump version to 1.0.18 | `43cff58e` | chore: bump version to 1.0.18 | resolved tray, history layout, branch docs/deps |
| 2026-06-01 | `integration/cuda` | `8ef91425` | chore: bump version to 1.0.18 | `5dcdb396` | chore: bump version to 1.0.18 | resolved history layout, branch docs/deps |
| 2026-06-01 | `release/microsoft-store` | `8ef91425` | chore: bump version to 1.0.18 | `8705dae4` | chore: bump version to 1.0.18 | runtime-only doc exclusion; lock patched |
| 2026-05-11 | `integration/combined` | `bf6cae7d` | chore: bump version to 1.0.17 | `304bdf24` | chore: bump version to 1.0.17 | resolved settings import conflict; combined release notes excluded |
| 2026-05-11 | `integration/cuda` | `bf6cae7d` | chore: bump version to 1.0.17 | `9c14061d` | chore: bump version to 1.0.17 | resolved settings import conflict; cuda notes updated |
| 2026-05-11 | `release/microsoft-store` | `bf6cae7d` | chore: bump version to 1.0.17 | `c91d72d0` | chore: bump version to 1.0.17 | clean cherry-pick; store notes updated |
| 2026-05-10 | `integration/combined` | `a2e3a115` | fix(settings): unify API key controls | `644f7ed7` | fix(settings): unify API key controls | clean cherry-pick |
| 2026-05-10 | `integration/cuda` | `a2e3a115` | fix(settings): unify API key controls | `225b6268` | fix(settings): unify API key controls | resolved model card conflict |
| 2026-05-10 | `release/microsoft-store` | `a2e3a115` | fix(settings): unify API key controls | `0710a8e3` | fix(settings): unify API key controls | clean cherry-pick |
| 2026-05-03 | `integration/combined` | `b476149b` | chore: bump version to 1.0.15 | `ecbfe423` | chore: bump version to 1.0.15 | resolved tray import, overlay, bindings/settings conflicts |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
