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
| 2026-03-13 | `cuda-integration` | `d984060` | fix(actions): quote CUDA workflow step names | `24f6c03` | fix(actions): quote CUDA workflow step names | direct branch-local equivalent; fixes invalid workflow YAML |
| 2026-03-13 | `Microsoft-store` | `70bf969` | chore(lockfile): refresh Cargo.lock for 1.0.2 | `186820e` | chore(lockfile): refresh Cargo.lock for 1.0.2 | same diff after local cargo check |
| 2026-03-13 | `Microsoft-store` | `3b1e4eb` | chore: bump version to 1.0.2 | `fa44ab1` | chore: bump version to 1.0.2 | clean cherry-pick |
| 2026-03-13 | `cuda-integration` | `0f5a937` | docs(connector): note local Axum server version | `8a303a8` | docs(connector): note local Axum server version | clean cherry-pick |
| 2026-03-13 | `Microsoft-store` | `0f5a937` | docs(connector): note local Axum server version | `de7de2f` | docs(connector): note local Axum server version | clean cherry-pick |
| 2026-03-13 | `cuda-integration` | `7fe0285` | chore(connector): refresh bundled extension to 1.0.5 | `2c16a88` | chore(connector): refresh bundled extension to 1.0.5 | clean cherry-pick |
| 2026-03-13 | `Microsoft-store` | `7fe0285` | chore(connector): refresh bundled extension to 1.0.5 | `2c70001` | chore(connector): refresh bundled extension to 1.0.5 | clean cherry-pick |
| 2026-03-13 | `cuda-integration` | `9486bf7` | feat(connector): improve export continuity, password syncing, and UX | `5fd5909` | feat(connector): improve export continuity, password syncing, and UX | clean cherry-pick |
| 2026-03-13 | `Microsoft-store` | `9486bf7` | feat(connector): improve export continuity, password syncing, and UX | `6f65a08` | feat(connector): improve export continuity, password syncing, and UX | clean cherry-pick |
| 2026-03-12 | `cuda-integration` | `e561c58` | fix(connector): decouple export restart and clarify status states | `49b002a` | fix(connector): decouple export restart and clarify status states | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
