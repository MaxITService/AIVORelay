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
| 2026-07-24 | `release/microsoft-store` | `3d772235` | chore: bump version to 1.0.26 | `777eedd2` | chore: bump version to 1.0.26 | 2 runtime fixes propagated; main-only docs and updater binding excluded; Store notes updated; lock version updated locally |
| 2026-07-22 | `release/microsoft-store` | `5b22f470` | chore: bump version to 1.0.25 | `c2d7c994` | chore: bump version to 1.0.25 | 15 runtime updates propagated; main-only docs and updater excluded; Store notes updated; lock regenerated locally |
| 2026-07-15 | `release/microsoft-store` | `86242355` | chore: bump version to 1.0.24 | `482f6bcc` | chore: bump version to 1.0.24 | 11 runtime commits propagated; main-only docs excluded; Store notes updated; lock resolved locally |
| 2026-07-12 | `release/microsoft-store` | `318a74dd` | chore: bump version to 1.0.23 | `7657ed89` | chore: bump version to 1.0.23 | runtime propagated; Store notes updated; lock resolved locally |
| 2026-06-24 | `release/microsoft-store` | `3b37e049` | chore: bump version to 1.0.22 | `a0121372` | chore: bump version to 1.0.22 | runtime clean; Store notes updated; lock patched |
| 2026-06-22 | `release/microsoft-store` | `00b053b9` | ci: pin Vulkan action to Node 24 cache fix | `a18d5198` | ci: pin Vulkan action to Node 24 cache fix | clean propagation |
| 2026-06-22 | `release/microsoft-store` | `630abf2e` | fix: repair release build and update Node actions | `18f7cebb` | fix: repair release build and update Node actions | clean propagation |
| 2026-06-22 | `release/microsoft-store` | `1e1f7379` | chore: bump version to 1.0.21 | `d2202bc4` | chore: bump version to 1.0.21 | store notes updated |
| 2026-06-18 | `release/microsoft-store` | `aed4b537` | chore: bump version to 1.0.20 | `7a2c8dfb` | chore: bump version to 1.0.20 | store notes updated |
| 2026-06-03 | `release/microsoft-store` | `9c9bc15b` | fix(installer): use executable icon for app shortcuts | `89d8fbf5` | fix(installer): use executable icon for app shortcuts | main notes excluded; store notes updated |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
