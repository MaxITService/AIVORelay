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
| 2026-03-13 | `cuda-integration` | `0f5a937` | docs(connector): note local Axum server version | `8a303a8` | docs(connector): note local Axum server version | clean cherry-pick |
| 2026-03-13 | `Microsoft-store` | `0f5a937` | docs(connector): note local Axum server version | `de7de2f` | docs(connector): note local Axum server version | clean cherry-pick |
| 2026-03-13 | `cuda-integration` | `7fe0285` | chore(connector): refresh bundled extension to 1.0.5 | `2c16a88` | chore(connector): refresh bundled extension to 1.0.5 | clean cherry-pick |
| 2026-03-13 | `Microsoft-store` | `7fe0285` | chore(connector): refresh bundled extension to 1.0.5 | `2c70001` | chore(connector): refresh bundled extension to 1.0.5 | clean cherry-pick |
| 2026-03-13 | `cuda-integration` | `9486bf7` | feat(connector): improve export continuity, password syncing, and UX | `5fd5909` | feat(connector): improve export continuity, password syncing, and UX | clean cherry-pick |
| 2026-03-13 | `Microsoft-store` | `9486bf7` | feat(connector): improve export continuity, password syncing, and UX | `6f65a08` | feat(connector): improve export continuity, password syncing, and UX | clean cherry-pick |
| 2026-03-12 | `cuda-integration` | `e561c58` | fix(connector): decouple export restart and clarify status states | `49b002a` | fix(connector): decouple export restart and clarify status states | clean cherry-pick |
| 2026-03-12 | `Microsoft-store` | `e561c58` | fix(connector): decouple export restart and clarify status states | `7eb4f05` | fix(connector): decouple export restart and clarify status states | clean cherry-pick |
| 2026-03-12 | `Microsoft-store` | `0efb3f7` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | `6f6e4e8` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | clean cherry-pick |
| 2026-03-12 | `cuda-integration` | `0efb3f7` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | `41c6348` | fix: dropdown opens upward when at bottom of group; menu widens to fit long options | clean cherry-pick |
| 2026-03-11 | `cuda-integration` | `6029d1e` | feat: show file transcription benchmark time | `a918c1b` | feat: show file transcription benchmark time | clean cherry-pick |
| 2026-03-11 | `Microsoft-store` | `5fc34cd` | chore: bump version to 1.0.1 | `ba38285` | chore: bump version to 1.0.1 | store release bump |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
