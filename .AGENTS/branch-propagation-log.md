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
| 2026-06-22 | `release/microsoft-store` | `00b053b9` | ci: pin Vulkan action to Node 24 cache fix | `a18d5198` | ci: pin Vulkan action to Node 24 cache fix | clean propagation |
| 2026-06-22 | `release/microsoft-store` | `630abf2e` | fix: repair release build and update Node actions | `18f7cebb` | fix: repair release build and update Node actions | clean propagation |
| 2026-06-22 | `release/microsoft-store` | `1e1f7379` | chore: bump version to 1.0.21 | `d2202bc4` | chore: bump version to 1.0.21 | store notes updated |
| 2026-06-18 | `release/microsoft-store` | `aed4b537` | chore: bump version to 1.0.20 | `7a2c8dfb` | chore: bump version to 1.0.20 | store notes updated |
| 2026-06-03 | `release/microsoft-store` | `9c9bc15b` | fix(installer): use executable icon for app shortcuts | `89d8fbf5` | fix(installer): use executable icon for app shortcuts | main notes excluded; store notes updated |
| 2026-06-01 | `release/microsoft-store` | `8ef91425` | chore: bump version to 1.0.18 | `8705dae4` | chore: bump version to 1.0.18 | runtime-only doc exclusion; lock patched |
| 2026-05-11 | `release/microsoft-store` | `bf6cae7d` | chore: bump version to 1.0.17 | `c91d72d0` | chore: bump version to 1.0.17 | clean cherry-pick; store notes updated |
| 2026-05-10 | `release/microsoft-store` | `a2e3a115` | fix(settings): unify API key controls | `0710a8e3` | fix(settings): unify API key controls | clean cherry-pick |
| 2026-05-03 | `release/microsoft-store` | `b476149b` | chore: bump version to 1.0.15 | `1a303b50` | chore: bump version to 1.0.15 | resolved bindings conflict; main release notes excluded |
| 2026-04-25 | `release/microsoft-store` | `4b6adfa5` | fix(overlay): keep error layout onscreen | `f8131ee0` | fix(overlay): keep error layout onscreen | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
