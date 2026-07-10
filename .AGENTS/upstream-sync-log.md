# Upstream Sync Log
Branch tags: #branch/main

Small rolling log of upstream commits integrated into `main`.

This file is maintained from `main` only.
Non-`main` branches must not carry or update independent copies.

Audit note (2026-07-10):
- Current fetched `upstream/main` head checked locally: `bf258d10`.
- Safe review cursor for the next `upstream -> main` intake: `0a59e1f3`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Reviewed corridor from `dad37baa` to `0a59e1f3`: manually adapted `0a59e1f3` (custom words with ampersands); skipped `45e3eed8` (Italian locale plus formatting-only Rust diff) and `cdb46339` (does not fit the fork preview-output architecture).
- Separately manually adapted `a201be91` (Handy Keys 0.3.0), `e2c72a25` (mic-level IPC throttling), and the remaining tray package portions from `2dd35bbb`/`bf258d10`; `5464bfaa` tray-state tracking was already covered by the fork. The review cursor remains unchanged because the preceding `66e57ca8` through `58760b22` decisions still need to be recorded.
- Explicitly skipped `fc465b49` (default LLM prompt injection defense) by product decision; no code port was made.
- Re-triaged corridor up to `fdc8cb71`: taken/logged `84d88f91`, `30b57c42`, `b123c1e`, `4609db7f`, `d1d33932`, `557d274d`, `17277cf6`, `58cda3f3`, `e35f0a71`, `cb32d35b`; already covered `095f4ac4`; skipped `fdc8cb71`, `c1697b2a`, `39e855de`, `743d8a54`, `8836d455`, `1a95c9c4`, `cd3ec3ab`, `c5ec92b3`, `e3c9f581`, `075a5887`, `012e0666`, `d33535cf`; treated `a3015026` as separate research / split adaptation, not a normal intake row.
- Re-triaged corridor from `fdc8cb71` to `564fbc84`: already covered `966ff997` by `cfb7a916`; skipped `f26fe0dc`, `0392b7b6`, `11311bee`, `564fbc84`.
- Re-triaged corridor from `564fbc84` to `af6ec6c9`: already covered `aee682f6` by `d225e59f`; skipped `a4d671a6`, `c1e11faa`, `af6ec6c9`.
- Reviewed corridor from `af6ec6c9` to `a385371c`: skipped `4b7bb4e5` (comment-only audio log clarification), `8346bc2d` (macOS/Nix build fix), `085cd530` (release bump), `a385371c` (Nix packaging refactor).
- Reviewed corridor from `a385371c` to `10a4c31b`: took `10a4c31b`; skipped `1d042f3e` (upstream agent docs), `e3206aa5` (Nix-only refactor), `933a5250` (Linux-only README workaround).
- Reviewed corridor from `10a4c31b` to `bc6a41e4`: took `dd6cc676`, `cfab1dda`, `bc6a41e4`; skipped `7901ef71` (Intel Mac build docs, missing fork `BUILD.md`).
- Reviewed corridor from `bc6a41e4` to `9b0d8a11`: took `c8eb33bc`; manually adapted `31d8fc24`, `bff4db7e`, `dc01346d`, `9b0d8a11`; already covered `a92a4d5e` by fork recording-overlay cache; skipped `e526733f` (debug log viewer feature), `8f722668` (upstream branding docs).
- Reviewed corridor from `9b0d8a11` to `dad37baa`: manually adapted `dad37baa` (GigaAM v3 catalog description).

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Main Message | Issues |
| --- | --- | --- | --- | --- | --- |
| 2026-07-10 | 2026-07-09 | `bf258d10` | fix: tray icon invisible on Windows with dark taskbar + light apps (#1423) (#1636) | fix(tray): preserve status and contrast on Windows | manual combined tray port; `5464bfaa` state tracking already covered; review cursor retained |
| 2026-07-10 | 2026-07-09 | `2dd35bbb` | fix(tray): log tray icon failures instead of panicking (#1355) | fix(tray): preserve status and contrast on Windows | manual combined tray port; added `set_icon` failure logging; review cursor retained |
| 2026-07-10 | 2026-07-08 | `e2c72a25` | fix: throttle mic-level IPC to mitigate WebKitWebProcess memory leak (#1444) | fix(overlay): throttle mic-level IPC | manual port; retained fork recording-overlay cache and single `emit_to` target; review cursor retained |
| 2026-07-10 | 2026-07-08 | `a201be91` | handy keys 0.3.0 (#1623) | chore(deps): update handy-keys to 0.3.0 | manual manifest port; `Cargo.lock` resolved locally with Cargo; review cursor retained |
| 2026-07-10 | 2026-07-07 | `0a59e1f3` | fix: preserve ampersands in custom words (#1569) | fix(custom-words): preserve ampersands | manual port; retained fork enabled/n-gram controls |
| 2026-07-07 | 2026-07-06 | `dad37baa` | Fix GigaAM v3 description. (#1613) | fix(models): correct GigaAM v3 catalog description | manual metadata port; no cherry-pick |
| 2026-07-01 | 2026-07-01 | `9b0d8a11` | edit model recs | feat(models): port upstream GGUF catalog | adapted via fork ModelInfo selector contract |
| 2026-07-01 | 2026-07-01 | `dc01346d` | update language selector | feat(models): expose catalog languages in selector | adapted to fork language constants/filtering |
| 2026-07-01 | 2026-06-25 | `bff4db7e` | debug + perf transcribe cli (#1541) | feat(cli): add headless transcription benchmark mode | adapted to fork CLI/startup flow |
| 2026-07-01 | 2026-06-30 | `31d8fc24` | feat: introduce transcribe.cpp / streaming (#1534) | feat(models): adapt transcribe.cpp and streaming preview | manual port; preserved fork preview pipeline |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | main commit message | short issue note |`
