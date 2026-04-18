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
| 2026-04-18 | `integration/combined` | `f36a1cdc` | chore(bindings): commit pending generated update | `0b555fcc` | chore(bindings): commit pending generated update | clean cherry-pick |
| 2026-04-18 | `integration/cuda` | `f36a1cdc` | chore(bindings): commit pending generated update | `143aa0d0` | chore(bindings): commit pending generated update | clean cherry-pick |
| 2026-04-18 | `release/microsoft-store` | `f36a1cdc` | chore(bindings): commit pending generated update | `a4363740` | chore(bindings): commit pending generated update | clean cherry-pick |
| 2026-04-18 | `integration/combined` | `7cf850fd` | fix(overlay): stabilize first paint and placement | `35e44f0d` | fix(overlay): stabilize first paint and placement | clean cherry-pick |
| 2026-04-18 | `integration/cuda` | `7cf850fd` | fix(overlay): stabilize first paint and placement | `d6b45f25` | fix(overlay): stabilize first paint and placement | clean cherry-pick |
| 2026-04-18 | `release/microsoft-store` | `7cf850fd` | fix(overlay): stabilize first paint and placement | `38b4c934` | fix(overlay): stabilize first paint and placement | clean cherry-pick |
| 2026-04-18 | `integration/combined` | `268ffdf9` | feat(about): add Reddit community link | `188b60cf` | feat(about): add Reddit community link | clean cherry-pick |
| 2026-04-18 | `integration/cuda` | `268ffdf9` | feat(about): add Reddit community link | `933d3c34` | feat(about): add Reddit community link | clean cherry-pick |
| 2026-04-18 | `release/microsoft-store` | `268ffdf9` | feat(about): add Reddit community link | `03dbcf52` | feat(about): add Reddit community link | clean cherry-pick |
| 2026-04-16 | `integration/combined` | `c7d56f56` | chore: bump version to 1.0.12 | `c7bf98ec` | chore: bump version to 1.0.12 | direct bump from branch 1.0.9 |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
