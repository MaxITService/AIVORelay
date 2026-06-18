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
| 2026-06-18 | `integration/cuda` | `aed4b537` | chore: bump version to 1.0.20 | `ba5c0ffb` | chore: bump version to 1.0.20 | resolved model settings import |
| 2026-06-03 | `integration/cuda` | `9c9bc15b` | fix(installer): use executable icon for app shortcuts | `506b2a0f` | chore: bump version to 1.0.19 | installer icon already matched |
| 2026-06-01 | `integration/cuda` | `8ef91425` | chore: bump version to 1.0.18 | `5dcdb396` | chore: bump version to 1.0.18 | resolved history layout, branch docs/deps |
| 2026-05-11 | `integration/cuda` | `bf6cae7d` | chore: bump version to 1.0.17 | `9c14061d` | chore: bump version to 1.0.17 | resolved settings import conflict; cuda notes updated |
| 2026-05-10 | `integration/cuda` | `a2e3a115` | fix(settings): unify API key controls | `225b6268` | fix(settings): unify API key controls | resolved model card conflict |
| 2026-05-03 | `integration/cuda` | `b476149b` | chore: bump version to 1.0.15 | `69bc618b` | chore: bump version to 1.0.15 | minimal docs; CUDA polish deferred |
| 2026-04-25 | `integration/cuda` | `4b6adfa5` | fix(overlay): keep error layout onscreen | `ff2e581c` | fix(overlay): keep error layout onscreen | resolved old geometry helper conflict |
| 2026-04-18 | `integration/combined` | `f36a1cdc` | chore(bindings): commit pending generated update | `0b555fcc` | chore(bindings): commit pending generated update | clean cherry-pick |
| 2026-04-18 | `integration/cuda` | `f36a1cdc` | chore(bindings): commit pending generated update | `143aa0d0` | chore(bindings): commit pending generated update | clean cherry-pick |
| 2026-04-18 | `release/microsoft-store` | `f36a1cdc` | chore(bindings): commit pending generated update | `a4363740` | chore(bindings): commit pending generated update | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
