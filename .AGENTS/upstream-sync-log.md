# Upstream Sync Log
Branch tags: #branch/main

Small rolling log of upstream commits integrated into `main`.

This file is maintained from `main` only.
Non-`main` branches must not carry or update independent copies.

Audit note (2026-07-22):
- Current fetched `upstream/main` head checked locally: `8a362e9`.
- Safe review cursor for the next `upstream -> main` intake: `8a362e9`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Reviewed corridor from `dad37baa` to `0a59e1f3`: manually adapted `0a59e1f3` (custom words with ampersands); skipped `45e3eed8` (Italian locale plus formatting-only Rust diff) and `cdb46339` (does not fit the fork preview-output architecture).
- Reviewed corridor from `0a59e1f3` to `bf258d10`: manually adapted `a201be91` (Handy Keys 0.3.0), `e2c72a25` (mic-level IPC throttling), `87c45f81` (transcribe.cpp 0.1.2), and the remaining tray package portions from `2dd35bbb`/`bf258d10`; already covered `eb9301e0` (resampler reset), `a6df7428` (poisoned-mutex recovery), `5464bfaa` (tray-state tracking), and `f79a907f` (fork session-generation/stale-result cancellation); treated `cd040d93` as superseded by the fork's backup/reset/user-notice recovery policy; skipped `66e57ca8` (Linux packaging), `485f4ade` (macOS build fallback), `58760b22` (optional translation), `11c2bb1e` (not needed with fork settings contract), `07637ea9` (logging-only), and `f0f7e7ff` (optional split paste-delay UX).
- Reviewed corridor from `bf258d10` to `38825767`: adapted `8c46721a` (Moonshine language descriptions) and `38825767` (onboarding download cancellation); already covered `1fd3f912` by the fork's global root Toaster; skipped `438582fc` (X11-only), `15816898` (upstream build documentation), `d1bc82a0` (merge commit), `d929a946` (appearance selector not needed for the fork's fixed dark UI), and `1c4f21ac` (release bump).
- Pre-adapted open upstream PRs on 2026-07-12: `#1645` (`3ddf255c`, Windows mixed-DPI monitor selection) into the fork's native overlay geometry path and `#1662` (`bb3fdda3`, active-model reselection guard) into the fork's compact model dropdown. Re-review both PRs after upstream merges in case their final patches change.
- Explicitly skipped `fc465b49` (default LLM prompt injection defense) by product decision; no code port was made.
- Re-triaged corridor up to `fdc8cb71`: taken/logged `84d88f91`, `30b57c42`, `b123c1e`, `4609db7f`, `d1d33932`, `557d274d`, `17277cf6`, `58cda3f3`, `e35f0a71`, `cb32d35b`; already covered `095f4ac4`; skipped `fdc8cb71`, `c1697b2a`, `39e855de`, `743d8a54`, `8836d455`, `1a95c9c4`, `cd3ec3ab`, `c5ec92b3`, `e3c9f581`, `075a5887`, `012e0666`, `d33535cf`; treated `a3015026` as separate research / split adaptation, not a normal intake row.
- Re-triaged corridor from `fdc8cb71` to `564fbc84`: already covered `966ff997` by `cfb7a916`; skipped `f26fe0dc`, `0392b7b6`, `11311bee`, `564fbc84`.
- Re-triaged corridor from `564fbc84` to `af6ec6c9`: already covered `aee682f6` by `d225e59f`; skipped `a4d671a6`, `c1e11faa`, `af6ec6c9`.
- Reviewed corridor from `af6ec6c9` to `a385371c`: skipped `4b7bb4e5` (comment-only audio log clarification), `8346bc2d` (macOS/Nix build fix), `085cd530` (release bump), `a385371c` (Nix packaging refactor).
- Reviewed corridor from `a385371c` to `10a4c31b`: took `10a4c31b`; skipped `1d042f3e` (upstream agent docs), `e3206aa5` (Nix-only refactor), `933a5250` (Linux-only README workaround).
- Reviewed corridor from `10a4c31b` to `bc6a41e4`: took `dd6cc676`, `cfab1dda`, `bc6a41e4`; skipped `7901ef71` (Intel Mac build docs, missing fork `BUILD.md`).
- Reviewed corridor from `bc6a41e4` to `9b0d8a11`: took `c8eb33bc`; manually adapted `31d8fc24`, `bff4db7e`, `dc01346d`, `9b0d8a11`; already covered `a92a4d5e` by fork recording-overlay cache; skipped `e526733f` (debug log viewer feature), `8f722668` (upstream branding docs).
- Reviewed corridor from `9b0d8a11` to `dad37baa`: manually adapted `dad37baa` (GigaAM v3 catalog description).
- Reviewed corridor from `38825767` to `ea10f74`: took `c912c6b` (`transcribe-cpp` 0.1.3 discrete-GPU priority and Windows build-path hardening); skipped `ea10f74` (release bump).
- Reviewed corridor from `68af495` to `8a362e9`: manually adapted `e8c73ba` (catalog and generator) and `8a362e9` (restore the user's prior system mute state); already covered `e1152d8` by the fork's native mixed-DPI overlay placement and `3ed2b21` by the fork's all-format clipboard restoration; skipped `17d6c76` (release bump), `cdf5028` (upstream sidebar refactor), `b462aa3` (optional tray-click behavior), and `f4e3587` (optional translation).

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Main Message | Issues |
| --- | --- | --- | --- | --- | --- |
| 2026-07-22 | 2026-07-22 | `8a362e9` | fix mute bug (#1760) | fix(audio): preserve system mute state after recording | manual port; retained fork capture-source checks and media pause/resume behavior |
| 2026-07-22 | 2026-07-21 | `e8c73ba` | update catalog | feat(models): add latest transcribe.cpp catalog models | manual catalog/generator port; kept all quantizations and fork Whisper translation support |
| 2026-07-22 | 2026-07-21 | `68af495` | dont use hf env | fix(models): ignore stale credentials for public downloads | manual client-builder port; retained fork cancellable parallel download implementation |
| 2026-07-22 | 2026-07-21 | `46d6a2a` | guard vulkan so only available to true x86 host, not emulated (#1731) | fix(acceleration): disable GGML GPU paths under x64-on-ARM | manual port; covered fork whisper.cpp and transcribe.cpp paths; fork workflow has no upstream package-smoke block |
| 2026-07-22 | 2026-07-21 | `0470d9a` | fix windows not shutting down cleanly because of handy (#1732) | fix(windows): allow clean system shutdown | manual tao patch port; retained fork Tauri 2.11.5 and resolved Cargo lock locally |
| 2026-07-22 | 2026-07-20 | `59ec695` | bump tauri global shortcut package.json | chore(deps): update frontend global-shortcut plugin to 2.3.2 | manual package and Bun lock port; retained fork dependency graph |
| 2026-07-22 | 2026-07-20 | `2281b8b` | bump tauri global shortcut | chore(deps): update Rust global-shortcut plugin to 2.3.2 | manual dependency resolution; Cargo lock resolved locally |
| 2026-07-22 | 2026-07-20 | `468129f` | bump to handy-keys 0.3.1 | chore(deps): update handy-keys to 0.3.1 | manual manifest port; Cargo lock resolved locally |
| 2026-07-22 | 2026-07-20 | `2203a82` | fix: prevent custom-word correction from losing transcriptions | fix(transcription): preserve results through text cleanup | manual port; retained fork n-gram/filler toggles; Unicode-safe matching and fail-open cleanup/headless guards |
| 2026-07-13 | 2026-07-12 | `c912c6b` | transcribe 0.1.3 (#1664) | chore(deps): update transcribe-cpp to 0.1.3 | manifest port; Cargo lock resolved locally; retained fork CI path workaround |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | main commit message | short issue note |`
