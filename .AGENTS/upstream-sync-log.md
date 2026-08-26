# Upstream Sync Log
Branch tags: #branch/main

Small rolling log of upstream commits integrated into `main`.

This file is maintained from `main` only.
Non-`main` branches must not carry or update independent copies.

Audit note (2026-08-24):
- Reviewed the complete linear corridor from `0e503672` through upstream HEAD
  `af48dd68`; the safe review cursor is now `af48dd68`. There are no merge
  commits in this corridor.
- Manually adapted `5ec2276a` so all native tray icon and menu changes use one
  coalescing main-thread writer while retaining AivoRelay's microphone picker,
  local and remote model menus, shortcut guide, and troubleshooting action.
- Updated both updater packages to 2.10.1 from `f6fac42e`, regenerating Bun and
  Cargo locks locally. Skipped `286e66cb` (generated-binding whitespace),
  `8fd66912` (Nix-only), and `af48dd68` (upstream release bump).

Audit note (2026-08-21):
- Reviewed the complete corridor after `99052ee`: `afbf44cd` (`transcribe.cpp
  0.2.0 (#1924)`), the parallel `d55ea7ef` (`drop 'gpu' accelerator selector`),
  and their conflict-resolution merge `0e503672`. The safe review cursor is now
  `0e503672`.
- Manually adapted the merge's final tree as `2e8ce179` (`feat(transcription):
  migrate to transcribe.cpp 0.2`). Both Rust crates are pinned to exact `0.2.0`;
  device selection now persists stable identities, migrates legacy indexes,
  reports requested and bound devices, and treats CLI registry index zero as an
  exact selection.
- Updated the v2 catalog generator to require pinned revisions and SHA-256
  metadata, enabled MOSS and PrimeLine, and kept transcription-less Sortformer
  models hidden. Deferred Multitalker's ambiguous nested bundle entries and
  duplicate quant names. Follow-up intake added Windows x64 dynamic-runtime
  staging/package audits and restored native ARM64's static CPU-only posture.
- Audited all six merge commits reachable from `upstream/main` in 2026. Five
  reproduce their automatic merge trees with no resolution-only behavior;
  `0e503672` is the only substantive merge resolution and is covered above.

Audit note (2026-08-18):
- Refreshed `Q:\Handy-upstream` and reviewed all 11 non-merge commits from
  `549cbde3` through upstream HEAD `99052ee`; the safe review cursor is now
  `99052ee`.
- Manually adapted `9e534a3` so portable builds keep default hf-hub model
  caches under `Data/huggingface` while retaining the fork's explicit local-TTS
  cache locations.
- Manually adapted `2cf157d` so the Custom Words UI accepts normalized
  multi-word phrases while retaining the fork's opt-in n-gram and ampersand
  correction behavior.
- Updated `handy-keys` from 0.3.3 to 0.3.4 for Pause/Break, PrintScreen,
  Context Menu, and additional keypad-key coverage on Windows.
- Manually adapted `c89b7bf` so asynchronous CPAL stream errors trigger a
  rebuild and a missing regular microphone falls back to the system default
  without disturbing clamshell, loopback, auto-switch, input-boost, noise-
  cancellation, or first-sample readiness behavior.
- Skipped optional Turkish translation cleanup, macOS-only input/runtime/docs,
  Bluetooth/macOS documentation, and the Linux/X11 modifier-latch fix.

Audit note (2026-08-17):
- Refreshed `Q:\Handy-upstream`; upstream HEAD is `8758dcc`, but this targeted
  review covers only `549cbde3`. The safe review cursor is now `549cbde3`.
- Manually adapted `549cbde3`: recording startup now waits for the first audio
  chunk before showing reactive levels, playing the start cue, or applying
  recording mute. A generation guard suppresses late readiness after Stop or
  Cancel, and the existing AivoRelay overlay shows a theme-compatible arming
  pulse while hardware is still starting.
- Retained AivoRelay's session ownership, input boost, noise cancellation,
  loopback capture, live-provider routing, custom overlay appearance system,
  and reduced-motion behavior. Skipped upstream's overlay replacement and
  development-only artificial readiness delay.

Audit note (2026-08-13):
- Reviewed the 11 non-merge commits from `db003f38` through `37a26fd6`; the safe
  review cursor is now `37a26fd6`. A later fetch advanced `upstream/main` to
  `549cbde3`, but that newer commit is outside this intake.
- Manually adapted `1bcbfc4c` by enabling reqwest gzip, Brotli, and deflate
  response decoding while retaining the fork's multipart support.
- Manually adapted the correctness portions of `4cd49950` and `80995b53`:
  filler removal now uses the actual requested, constrained, detected, or
  translated output language across local, remote, realtime, segmented, and
  diarized file paths. Retained the fork's opt-in default, custom n-grams, and
  hallucination cleanup policy; skipped upstream's default-on setting/UI,
  translations, and generated bindings.
- Skipped Linux-only input/paste/overlay work, Ubuntu documentation, optional
  translation and typo changes, upstream appearance theming, and macOS-only
  Secure Input/tray behavior.

Audit note (2026-08-08):
- Refreshed `Q:\Handy-upstream`; current upstream HEAD is `db003f38`.
- Safe review cursor for the next `upstream -> main` intake is now `db003f38`.
- Reviewed the full corridor from `76736d5` to `db003f38` (21 non-merge commits).
- Integrated or manually adapted nine Windows/core fixes: capture-worker recovery,
  non-blocking CPAL access and lock-free recording status, explicit non-streaming
  post-processing requests, preserved HTTP error causes, hidden-overlay rendering,
  Windows microphone-permission precedence, multi-channel microphone selection,
  the `js-yaml` security update, and idle always-on audio processing.
- `09aaf4d3` was already covered by the fork's authoritative final-text handling.
  `b1b2d9f9` targets an upstream paste subsystem absent from this fork, and
  `6d3239e0` targets an absent What's New screen. Skipped Linux/Nix/AppImage,
  upstream housekeeping/release commits, and the optional model-filter UX.

Audit note (2026-08-01):
- Refreshed `Q:\Handy-upstream`; current upstream HEAD is `76736d5`.
- Safe review cursor for the next `upstream -> main` intake is now `76736d5`.
- Reviewed the full corridor from `8a362e9` to `76736d5` (21 non-merge commits).
- Manually adapted `292db647` as `55176cae` (pinned catalog revisions,
  mirror metadata, SHA-256 verification, resumable/stall-aware downloads) and
  `16e5d48e` as `310c8619` (suspend all active bindings during shortcut
  capture).
- `70327582` and `76b44d83` are included in `6dfe700b`; `148e5492` is already
  covered independently. Skipped macOS-only `d001fcd9`, `0902937b`, and
  `f602ef4f` because AIVORelay does not ship macOS. Deferred `a70ac84f` and
  `cf49ab35` for workflow/platform-specific follow-up.

Audit note (2026-07-22):
- Current fetched `upstream/main` head checked locally: `8a362e9`.
- Safe review cursor for the next `upstream -> main` intake: `8a362e9`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Reviewed corridor from `dad37baa` to `0a59e1f3`: manually adapted `0a59e1f3` (custom words with ampersands); skipped `45e3eed8` (Italian locale plus formatting-only Rust diff) and `cdb46339` (does not fit the fork preview-output architecture).
- Reviewed corridor from `0a59e1f3` to `bf258d10`: manually adapted `a201be91` (Handy Keys 0.3.0), `e2c72a25` (mic-level IPC throttling), `87c45f81` (transcribe.cpp 0.1.2), and the remaining tray package portions from `2dd35bbb`/`bf258d10`; already covered `eb9301e0` (resampler reset), `a6df7428` (poisoned-mutex recovery), `5464bfaa` (tray-state tracking), and `f79a907f` (fork session-generation/stale-result cancellation); treated `cd040d93` as superseded by the fork's backup/reset/user-notice recovery policy; skipped `66e57ca8` (Linux packaging), `485f4ade` (macOS build fallback), `58760b22` (optional translation), `11c2bb1e` (not needed with fork settings contract), `07637ea9` (logging-only), and `f0f7e7ff` (optional split paste-delay UX).
- Reviewed corridor from `bf258d10` to `38825767`: adapted `8c46721a` (Moonshine language descriptions) and `38825767` (onboarding download cancellation); already covered `1fd3f912` by the fork's global root Toaster; skipped `438582fc` (X11-only), `15816898` (upstream build documentation), `d1bc82a0` (merge commit), `d929a946` (appearance selector not needed for the fork's fixed dark UI), and `1c4f21ac` (release bump).
- Pre-adapted open upstream PRs on 2026-07-12: `#1645` (`3ddf255c`, Windows mixed-DPI monitor selection) into the fork's native overlay geometry path and `#1662` (`bb3fdda3`, active-model reselection guard) into the fork's compact model dropdown. Re-review both PRs after upstream merges in case their final patches change.
- Pre-adapted open upstream PR on 2026-07-22: `#1740` (`e9934a6e`, persistent audio-feedback stream with bounded playback waits) while retaining the fork's independent result-ready cue. Re-review after merge in case the final patch changes.
- Pre-adapted open upstream PR on 2026-07-22: `#1753` (`931ac27d`, direct portable-update installer link) using AivoRelay's signed x64 MSI asset naming and existing portable `Data` contract. Re-review after merge in case the final patch changes.
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
| 2026-08-24 | 2026-08-23 | `f6fac42e` | update to tauri-plugin-updater 2.10.1 | fix(deps): update Tauri updater to 2.10.1 | manifests updated; Bun and Cargo locks regenerated locally; Nix metadata skipped |
| 2026-08-24 | 2026-08-23 | `5ec2276a` | single writer tray icon (#1952) | fix(tray): serialize native tray updates | manual desired-state port; retained fork microphone, provider, shortcut-guide, and troubleshooting menus |
| 2026-08-21 | 2026-08-19 | `afbf44cd`, `d55ea7ef`, `0e503672` | transcribe.cpp 0.2.0 (#1924); drop 'gpu' accelerator selector; Merge branch 'main' of github.com:cjpais/Handy | feat(transcription): migrate to transcribe.cpp 0.2 | manual final-tree adaptation; merge resolution reviewed; Windows runtime packaging completed; Multitalker bundle deferred |
| 2026-08-18 | 2026-08-17 | `c89b7bf` | fix: fall back to default microphone after disconnect (#1874) | fix(audio): recover after microphone disconnects | manual health/fallback port; retained fork capture routing and audio processing |
| 2026-08-18 | 2026-08-17 | `5c77861` | bump handy-keys to 0.3.4 | fix(deps): update handy-keys to 0.3.4 | manifest update; lock refreshed locally without taking upstream lock |
| 2026-08-18 | 2026-08-17 | `2cf157d` | fix(custom-words): allow multi-word phrases (#1406) | fix(custom-words): allow multi-word phrases | manual UI/test port; retained fork n-gram toggle and ampersand handling |
| 2026-08-18 | 2026-08-14 | `9e534a3` | fix(portable): keep Hugging Face models in Data (#1908) | fix(portable): keep Hugging Face models in Data | manual portable-init port; retained fork-managed cache paths |
| 2026-08-17 | 2026-08-13 | `549cbde3` | add a better viz for mic waiting | feat(audio): show capture readiness from first samples | manual readiness/session/UI port; retained fork capture sources and overlay themes |
| 2026-08-13 | 2026-08-09 | `4cd49950`, `80995b53` | wip filler fixes (#1738); forgot to commit bindings | fix(transcription): make filler removal output-language aware | manual correctness port; retained fork opt-in/custom-word policy and all transcription paths |
| 2026-08-13 | 2026-08-09 | `1bcbfc4c` | fix: support compressed API responses (#1548) | fix(http): support compressed provider responses | manual Cargo feature port; retained multipart support; lock regenerated locally |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | main commit message | short issue note |`
