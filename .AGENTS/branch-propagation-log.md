# Branch Propagation Log
Branch tags: #branch/main #branch/microsoft-store #branch/cuda-integration #branch/codex-combined

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
| 2026-04-10 | `codex/combined` | `6ff33177` | fix(window): ignore minimized saved geometry | `56b87b3d` | fix(window): ignore minimized saved geometry | clean cherry-pick |
| 2026-04-10 | `cuda-integration` | `6ff33177` | fix(window): ignore minimized saved geometry | `b7e96bbe` | fix(window): ignore minimized saved geometry | clean cherry-pick |
| 2026-04-10 | `Microsoft-store` | `6ff33177` | fix(window): ignore minimized saved geometry | `b70e6e4b` | fix(window): ignore minimized saved geometry | clean cherry-pick |
| 2026-04-09 | `codex/combined` | `0fd51a6f` | perf(post-processing): disable default reasoning on compatible providers | `e5eda05d` | perf(post-processing): disable default reasoning on compatible providers | kept local Cargo.lock; excluded upstream-sync-log |
| 2026-04-09 | `cuda-integration` | `0fd51a6f` | perf(post-processing): disable default reasoning on compatible providers | `3cea8f07` | perf(post-processing): disable default reasoning on compatible providers | excluded Cargo.lock + upstream-sync-log |
| 2026-04-09 | `Microsoft-store` | `0fd51a6f` | perf(post-processing): disable default reasoning on compatible providers | `6b79473d` | perf(post-processing): disable default reasoning on compatible providers | excluded Cargo.lock + upstream-sync-log |
| 2026-04-05 | `Microsoft-store` | `9cf150e4` | better algorithm: transcribe file recording | `8c0cc842` | better algorithm: transcribe file recording | skipped empty version bump |
| 2026-04-05 | `cuda-integration` | `9cf150e4` | better algorithm: transcribe file recording | `5e7b80bf` | better algorithm: transcribe file recording | solved Cargo.toml conflict |
| 2026-04-05 | `codex/combined` | `9cf150e4` | better algorithm: transcribe file recording | `8ffe01c4` | better algorithm: transcribe file recording | skipped empty version bump |
| 2026-04-04 | `Microsoft-store` | `5752185f` | change default model unload timeout to 15 minutes | `f7044b6b` | change default model unload timeout to 15 minutes | includes 09c0b163 + 943cd525 |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
