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
| 2026-06-22 | `integration/combined` | `630abf2e` | fix: repair release build and update Node actions | `98e2e09a` | fix: repair release build and update Node actions | clean propagation |
| 2026-06-22 | `integration/cuda` | `630abf2e` | fix: repair release build and update Node actions | `7db52d68` | fix: repair release build and update Node actions | clean propagation |
| 2026-06-22 | `release/microsoft-store` | `630abf2e` | fix: repair release build and update Node actions | `18f7cebb` | fix: repair release build and update Node actions | clean propagation |
| 2026-06-22 | `integration/combined` | `1e1f7379` | chore: bump version to 1.0.21 | `501d2108` | chore: bump version to 1.0.21 | clean propagation |
| 2026-06-22 | `integration/cuda` | `1e1f7379` | chore: bump version to 1.0.21 | `35da4d00` | chore: bump version to 1.0.21 | preserved CUDA accelerator migration |
| 2026-06-22 | `release/microsoft-store` | `1e1f7379` | chore: bump version to 1.0.21 | `d2202bc4` | chore: bump version to 1.0.21 | store notes updated |
| 2026-06-18 | `integration/combined` | `aed4b537` | chore: bump version to 1.0.20 | `c6b21c92` | chore: bump version to 1.0.20 | clean propagation |
| 2026-06-18 | `integration/cuda` | `aed4b537` | chore: bump version to 1.0.20 | `ba5c0ffb` | chore: bump version to 1.0.20 | resolved model settings import |
| 2026-06-18 | `release/microsoft-store` | `aed4b537` | chore: bump version to 1.0.20 | `7a2c8dfb` | chore: bump version to 1.0.20 | store notes updated |
| 2026-06-03 | `integration/combined` | `9c9bc15b` | fix(installer): use executable icon for app shortcuts | `bf18a8c0` | chore: bump version to 1.0.19 | installer icon already matched |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
