# Upstream Sync Log
Branch tags: #branch/main

Small rolling log of upstream commits integrated into `main`.

This file is maintained from `main` only.
Non-`main` branches must not carry or update independent copies.

Audit note (2026-04-30):
- Current fetched `upstream/main` head checked locally: `bc6a41e4`.
- Safe review cursor for the next `upstream -> main` intake: `bc6a41e4`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Re-triaged corridor up to `fdc8cb71`: taken/logged `84d88f91`, `30b57c42`, `b123c1e`, `4609db7f`, `d1d33932`, `557d274d`, `17277cf6`, `58cda3f3`, `e35f0a71`, `cb32d35b`; already covered `095f4ac4`; skipped `fdc8cb71`, `c1697b2a`, `39e855de`, `743d8a54`, `8836d455`, `1a95c9c4`, `cd3ec3ab`, `c5ec92b3`, `e3c9f581`, `075a5887`, `012e0666`, `d33535cf`; treated `a3015026` as separate research / split adaptation, not a normal intake row.
- Re-triaged corridor from `fdc8cb71` to `564fbc84`: already covered `966ff997` by `cfb7a916`; skipped `f26fe0dc`, `0392b7b6`, `11311bee`, `564fbc84`.
- Re-triaged corridor from `564fbc84` to `af6ec6c9`: already covered `aee682f6` by `d225e59f`; skipped `a4d671a6`, `c1e11faa`, `af6ec6c9`.
- Reviewed corridor from `af6ec6c9` to `a385371c`: skipped `4b7bb4e5` (comment-only audio log clarification), `8346bc2d` (macOS/Nix build fix), `085cd530` (release bump), `a385371c` (Nix packaging refactor).
- Reviewed corridor from `a385371c` to `10a4c31b`: took `10a4c31b`; skipped `1d042f3e` (upstream agent docs), `e3206aa5` (Nix-only refactor), `933a5250` (Linux-only README workaround).
- Reviewed corridor from `10a4c31b` to `bc6a41e4`: took `dd6cc676`, `cfab1dda`, `bc6a41e4`; skipped `7901ef71` (Intel Mac build docs, missing fork `BUILD.md`).

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Main Message | Issues |
| --- | --- | --- | --- | --- | --- |
| 2026-06-18 | 2026-06-18 | `bc6a41e4` | fix: dropdown overflow in post-processing settings (#1402) | fix(ui): prevent post-processing dropdown overflow | manual port to fork dropdown styles |
| 2026-06-18 | 2026-06-11 | `cfab1dda` | fix(models): show size for downloaded models (#1484) | fix(models): show size for downloaded models | adapted to fork settings/dropdown UI |
| 2026-06-18 | 2026-06-10 | `dd6cc676` | fix(visualizer): scale FFT window to device sample rate (#1491) | fix(visualizer): scale FFT window to device sample rate | manual port; preserved fork audio pipeline |
| 2026-05-23 | 2026-05-23 | `10a4c31b` | docs: complete Portuguese translation for portable updates (#1422) | docs: complete Portuguese translation for portable updates (#1422) | clean cherry-pick after locale conflict |
| 2026-04-14 | 2026-04-13 | `aee682f6` | feat: add AWS Bedrock (Mantle) as post-processing provider (#1288) | feat(post-processing): add AWS Bedrock Mantle provider | adapted provider fields to fork shape |
| 2026-04-09 | 2026-04-07 | `84d88f91` | perf: add reasoning_effort passthrough to avoid thinking-mode latency in local models (#1221) | perf(post-processing): disable default reasoning on compatible providers | adapted; preserved fork reasoning controls |
| 2026-04-09 | 2026-04-07 | `30b57c42` | fix(issue 522): surface paste errors as UI toast notification (#1198) | fix(ui): surface transcription paste failures as toast | adapted to fork App/error flow |
| 2026-04-02 | 2026-04-02 | `b123c1e5` | fix crash on old cpus (#1176) | fix: accept upstream old CPU crash fix | manual port; forgot log on first pass |
| 2026-04-01 | 2026-04-01 | `4609db7f` | add cohere (#1200) | feat(transcription): port upstream Cohere support | partial port; kept existing dirty bindings untouched |
| 2026-03-30 | 2026-03-29 | `d1d33932` | upgrade transcribe rs to 0.3.5 (#1188) | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; Canary default-fill already present in fork |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | main commit message | short issue note |`
