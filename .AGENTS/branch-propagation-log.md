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
| 2026-04-13 | `integration/combined` | `28a42507` | chore(bindings): drop accelerator discovery doc comment | `b6ea3b28` | chore(bindings): drop accelerator discovery doc comment | clean cherry-pick |
| 2026-04-13 | `integration/cuda` | `28a42507` | chore(bindings): drop accelerator discovery doc comment | `8f661f21` | chore(bindings): drop accelerator discovery doc comment | clean cherry-pick |
| 2026-04-13 | `release/microsoft-store` | `28a42507` | chore(bindings): drop accelerator discovery doc comment | `3211157b` | chore(bindings): drop accelerator discovery doc comment | clean cherry-pick |
| 2026-04-10 | `integration/combined` | `6ff33177` | fix(window): ignore minimized saved geometry | `56b87b3d` | fix(window): ignore minimized saved geometry | clean cherry-pick |
| 2026-04-10 | `integration/cuda` | `6ff33177` | fix(window): ignore minimized saved geometry | `b7e96bbe` | fix(window): ignore minimized saved geometry | clean cherry-pick |
| 2026-04-10 | `release/microsoft-store` | `6ff33177` | fix(window): ignore minimized saved geometry | `b70e6e4b` | fix(window): ignore minimized saved geometry | clean cherry-pick |
| 2026-04-09 | `integration/combined` | `0fd51a6f` | perf(post-processing): disable default reasoning on compatible providers | `e5eda05d` | perf(post-processing): disable default reasoning on compatible providers | kept local Cargo.lock; excluded upstream-sync-log |
| 2026-04-09 | `integration/cuda` | `0fd51a6f` | perf(post-processing): disable default reasoning on compatible providers | `3cea8f07` | perf(post-processing): disable default reasoning on compatible providers | excluded Cargo.lock + upstream-sync-log |
| 2026-04-09 | `release/microsoft-store` | `0fd51a6f` | perf(post-processing): disable default reasoning on compatible providers | `6b79473d` | perf(post-processing): disable default reasoning on compatible providers | excluded Cargo.lock + upstream-sync-log |
| 2026-04-05 | `release/microsoft-store` | `9cf150e4` | better algorithm: transcribe file recording | `8c0cc842` | better algorithm: transcribe file recording | skipped empty version bump |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
