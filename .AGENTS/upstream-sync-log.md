# Upstream Sync Log
Branch tags: #branch/main

Small rolling log of upstream commits integrated into `main`.

This file is maintained from `main` only.
Non-`main` branches must not carry or update independent copies.

Audit note (2026-04-09):
- Current fetched `upstream/main` head checked locally: `fdc8cb71`.
- Safe review cursor for the next `upstream -> main` intake: `fdc8cb71`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Re-triaged corridor up to `fdc8cb71`: taken/logged `84d88f91`, `30b57c42`, `b123c1e`, `4609db7f`, `d1d33932`, `557d274d`, `17277cf6`, `58cda3f3`, `e35f0a71`, `cb32d35b`; already covered `095f4ac4`; skipped `fdc8cb71`, `c1697b2a`, `39e855de`, `743d8a54`, `8836d455`, `1a95c9c4`, `cd3ec3ab`, `c5ec92b3`, `e3c9f581`, `075a5887`, `012e0666`, `d33535cf`; treated `a3015026` as separate research / split adaptation, not a normal intake row.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Main Message | Issues |
| --- | --- | --- | --- | --- | --- |
| 2026-04-09 | 2026-04-07 | `84d88f91` | perf: add reasoning_effort passthrough to avoid thinking-mode latency in local models (#1221) | perf(post-processing): disable default reasoning on compatible providers | adapted; preserved fork reasoning controls |
| 2026-04-09 | 2026-04-07 | `30b57c42` | fix(issue 522): surface paste errors as UI toast notification (#1198) | fix(ui): surface transcription paste failures as toast | adapted to fork App/error flow |
| 2026-04-02 | 2026-04-02 | `b123c1e5` | fix crash on old cpus (#1176) | fix: accept upstream old CPU crash fix | manual port; forgot log on first pass |
| 2026-04-01 | 2026-04-01 | `4609db7f` | add cohere (#1200) | feat(transcription): port upstream Cohere support | partial port; kept existing dirty bindings untouched |
| 2026-03-30 | 2026-03-29 | `d1d33932` | upgrade transcribe rs to 0.3.5 (#1188) | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; Canary default-fill already present in fork |
| 2026-03-25 | 2026-03-23 | `557d274d` | release v0.8.0 | fix(audio): surface no-input-device errors | partial intake split across `68fa267f`, `83295a35`, `152c2c1e` |
| 2026-03-22 | 2026-03-22 | `17277cf6` | Save recordings before transcription (#1024) | feat(history): save recordings before transcription | adapted via diff path; retry limited to transcription entries |
| 2026-03-21 | 2026-03-21 | `58cda3f3` | fix: sha256 verification to prevent corrupt partial download loop (#1095) | fix(models): verify downloads and clear corrupt partials | partial port; runtime fix kept, UI/state handling simplified |
| 2026-03-21 | 2026-03-21 | `e35f0a71` | improve history performance (#1107) | perf(history): paginate history settings list | adapted for fork history fields; raw invoke/listen instead of bindings |
| 2026-03-21 | 2026-03-19 | `cb32d35b` | feat(audio): lazy stream close for bluetooth mic latency (#747) | feat(audio): add lazy mic stream close toggle | adapted; backend intent kept, Debug UI/bindings diverged |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | main commit message | short issue note |`
