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
| 2026-04-13 | `integration/combined` | `f4532462` | chore(settings): move history controls to history page | `1f21cf32` | chore(settings): move history controls to history page | clean cherry-pick |
| 2026-04-13 | `integration/cuda` | `f4532462` | chore(settings): move history controls to history page | `79b0f8c9` | chore(settings): move history controls to history page | clean cherry-pick |
| 2026-04-13 | `release/microsoft-store` | `f4532462` | chore(settings): move history controls to history page | `ed0e33fa` | chore(settings): move history controls to history page | clean cherry-pick |
| 2026-04-13 | `integration/combined` | `2dabd749` | feat(audio): pause or mute output while recording | `0e661aae` | feat(audio): pause or mute output while recording | clean cherry-pick |
| 2026-04-13 | `integration/cuda` | `2dabd749` | feat(audio): pause or mute output while recording | `adeeca6b` | feat(audio): pause or mute output while recording | clean cherry-pick |
| 2026-04-13 | `release/microsoft-store` | `2dabd749` | feat(audio): pause or mute output while recording | `e42bb569` | feat(audio): pause or mute output while recording | clean cherry-pick |
| 2026-04-13 | `integration/combined` | `28a42507` | chore(bindings): drop accelerator discovery doc comment | `b6ea3b28` | chore(bindings): drop accelerator discovery doc comment | clean cherry-pick |
| 2026-04-13 | `integration/cuda` | `28a42507` | chore(bindings): drop accelerator discovery doc comment | `8f661f21` | chore(bindings): drop accelerator discovery doc comment | clean cherry-pick |
| 2026-04-13 | `release/microsoft-store` | `28a42507` | chore(bindings): drop accelerator discovery doc comment | `3211157b` | chore(bindings): drop accelerator discovery doc comment | clean cherry-pick |
| 2026-04-10 | `integration/combined` | `6ff33177` | fix(window): ignore minimized saved geometry | `56b87b3d` | fix(window): ignore minimized saved geometry | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
