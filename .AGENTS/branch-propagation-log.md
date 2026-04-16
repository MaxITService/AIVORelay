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
| 2026-04-16 | `integration/combined` | `c7d56f56` | chore: bump version to 1.0.12 | `c7bf98ec` | chore: bump version to 1.0.12 | direct bump from branch 1.0.9 |
| 2026-04-16 | `integration/cuda` | `c7d56f56` | chore: bump version to 1.0.12 | `46909845` | chore: bump version to 1.0.12 | direct bump from branch 1.0.9 |
| 2026-04-16 | `release/microsoft-store` | `c7d56f56` | chore: bump version to 1.0.12 | `691fb7aa` | chore: bump version to 1.0.12 | direct bump from branch 1.0.9 |
| 2026-04-16 | `integration/combined` | `1dcaffd5` | Fix file transcription cancellation feedback | `180c529e` | Fix file transcription cancellation feedback | clean cherry-pick |
| 2026-04-16 | `integration/cuda` | `1dcaffd5` | Fix file transcription cancellation feedback | `b5398a6a` | Fix file transcription cancellation feedback | clean cherry-pick |
| 2026-04-16 | `release/microsoft-store` | `1dcaffd5` | Fix file transcription cancellation feedback | `dace24ec` | Fix file transcription cancellation feedback | clean cherry-pick |
| 2026-04-16 | `integration/combined` | `d225e59f` | feat(post-processing): add AWS Bedrock Mantle provider | `28c58d3b` | feat(post-processing): add AWS Bedrock Mantle provider | main-only sync log excluded |
| 2026-04-16 | `integration/cuda` | `d225e59f` | feat(post-processing): add AWS Bedrock Mantle provider | `f55a9b8a` | feat(post-processing): add AWS Bedrock Mantle provider | main-only sync log excluded |
| 2026-04-16 | `release/microsoft-store` | `d225e59f` | feat(post-processing): add AWS Bedrock Mantle provider | `852109c9` | feat(post-processing): add AWS Bedrock Mantle provider | main-only sync log excluded |
| 2026-04-14 | `integration/combined` | `40531cd7` | Cap Cohere file transcription chunks | `b84259cb` | Cap Cohere file transcription chunks | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
