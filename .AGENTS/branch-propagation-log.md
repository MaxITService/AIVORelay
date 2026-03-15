# Branch Propagation Log

Small rolling log of `main` commits propagated into non-`main` release branches.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per branch propagation event.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Propagation Date | Target Branch | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-15 | `cuda-integration` | `ca08fe72` | feat(settings): repair invalid settings and bump version to 1.0.3 | `fc78b7b6` | feat(settings): repair invalid settings and bump version to 1.0.3 | clean cherry-pick |
| 2026-03-15 | `Microsoft-store` | `ca08fe72` | feat(settings): repair invalid settings and bump version to 1.0.3 | `c0fbd1c3` | feat(settings): repair invalid settings and bump version to 1.0.3 | clean cherry-pick |
| 2026-03-14 | `cuda-integration` | `7d594c0` | Fix immediate model switching state | `9c2f4f7` | Fix immediate model switching state | clean cherry-pick |
| 2026-03-14 | `Microsoft-store` | `7d594c0` | Fix immediate model switching state | `4a69ba3` | Fix immediate model switching state | clean cherry-pick |
| 2026-03-14 | `cuda-integration` | `019d9ab` | Fix post-intake compile issues | `fb014a0` | Fix post-intake compile issues | clean cherry-pick |
| 2026-03-14 | `Microsoft-store` | `019d9ab` | Fix post-intake compile issues | `bd026da` | Fix post-intake compile issues | clean cherry-pick |
| 2026-03-14 | `cuda-integration` | `f3d8c86` | Add Windows microphone permission onboarding | `2444150` | Add Windows microphone permission onboarding | handy-keys pick was empty/equivalent; portable conflict resolved; kept CUDA title in local follow-up |
| 2026-03-14 | `Microsoft-store` | `f3d8c86` | Add Windows microphone permission onboarding | `c4baa60` | Add Windows microphone permission onboarding | portable conflict resolved; kept Store title in local follow-up |
| 2026-03-13 | `cuda-integration` | `d984060` | fix(actions): quote CUDA workflow step names | `24f6c03` | fix(actions): quote CUDA workflow step names | direct branch-local equivalent; fixes invalid workflow YAML |
| 2026-03-13 | `Microsoft-store` | `70bf969` | chore(lockfile): refresh Cargo.lock for 1.0.2 | `186820e` | chore(lockfile): refresh Cargo.lock for 1.0.2 | same diff after local cargo check |
| 2026-03-13 | `Microsoft-store` | `3b1e4eb` | chore: bump version to 1.0.2 | `fa44ab1` | chore: bump version to 1.0.2 | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
