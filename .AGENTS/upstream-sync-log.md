# Upstream Sync Log

Small rolling log of upstream commits integrated into `main`.

Audit note (2026-03-25):
- Current fetched `upstream/main` head checked locally: `d1d33932`.
- Safe review cursor for the next `upstream -> main` intake: `d1d33932`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Re-triaged corridor up to `d1d33932`: taken/logged `d1d33932`, `557d274d`, `17277cf6`, `58cda3f3`, `e35f0a71`, `cb32d35b`, `0b3322fa`, `e1a484f7`, `5a3e6e33`, `2eeb2129`; already covered `095f4ac4`; skipped `8836d455`, `1a95c9c4`, `cd3ec3ab`, `c5ec92b3`, `e3c9f581`, `075a5887`, `012e0666`, `d33535cf`; treated `a3015026` as separate research / split adaptation, not a normal intake row.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Local SHA | Local Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-30 | 2026-03-29 | `d1d33932` | upgrade transcribe rs to 0.3.5 (#1188) | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; Canary default-fill already present in fork |
| 2026-03-25 | 2026-03-23 | `557d274d` | release v0.8.0 | `152c2c1e` | fix(audio): surface no-input-device errors | partial intake split across `68fa267f`, `83295a35`, `152c2c1e` |
| 2026-03-22 | 2026-03-22 | `17277cf6` | Save recordings before transcription (#1024) | `f5d15bc9` | feat(history): save recordings before transcription | adapted via diff path; retry limited to transcription entries |
| 2026-03-21 | 2026-03-21 | `58cda3f3` | fix: sha256 verification to prevent corrupt partial download loop (#1095) | `4d4b46db` | fix(models): verify downloads and clear corrupt partials | partial port; runtime fix kept, UI/state handling simplified |
| 2026-03-21 | 2026-03-21 | `e35f0a71` | improve history performance (#1107) | `f0eb727b` | perf(history): paginate history settings list | adapted for fork history fields; raw invoke/listen instead of bindings |
| 2026-03-21 | 2026-03-19 | `cb32d35b` | feat(audio): lazy stream close for bluetooth mic latency (#747) | `54696496` | feat(audio): add lazy mic stream close toggle | adapted; backend intent kept, Debug UI/bindings diverged |
| 2026-03-21 | 2026-03-19 | `0b3322fa` | feat(audio): use device default sample rate and always downsample (#1084) | `7c6b59f6` | feat(audio): prefer device default mic sample rate | well taken for mic path; loopback kept fork-specific |
| 2026-03-18 | 2026-03-18 | `e1a484f7` | ci: reduce PR check time from ~30 min to ~1 min (#1073) | `5dc33a2d` | Consolidate PR code quality checks | partial port only; merged lint+prettier with concurrency/path filters, skipped nix/playwright/test pieces |
| 2026-03-18 | 2026-03-18 | `5a3e6e33` | add extra recording buffer (#1089) | `ba2bba60` | Add local-only extra recording buffer | partial port only; kept remote Soniox/Deepgram buffer behavior unchanged |
| 2026-03-18 | 2026-03-18 | `2eeb2129` | upgrade path from old giga-am to new (#1088) | `25a03b17` | Port GigaAM v3 directory migration | adapted onto fork transcribe-rs 0.3.x state; added vocab resource + old-format migration |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | 'local_sha' | local message | short issue note |`
