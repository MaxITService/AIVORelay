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
| 2026-11-25 | `release/microsoft-store` | `a177a84cb` | feat(benchmark): make reference text easy to find and set from results | `8f59911d5` | feat(benchmark): make reference text easy to find and set from results | Ten ordered picks. TESTING and README changes excluded. Shared source matched after updating What's New for 1.0.57. |
| 2026-11-24 | `release/microsoft-store` | `931baa9c5` | chore(bindings): remove the retired macOS permission reset command | `7599805cf` | chore(bindings): remove the retired macOS permission reset command | Independently generated bindings matched byte for byte. Both branches passed 433 frontend tests and TypeScript. |
| 2026-11-24 | `release/microsoft-store` | `4c76b3dd4` | fix(macos): request onboarding permissions without resetting them | `4066f1ea6` | fix(macos): request onboarding permissions without resetting them | Four ordered picks. Both branches passed 1,439 Rust tests (28 ignored), 433 frontend tests, TypeScript and release translation checks. macOS prompts unverified. |
| 2026-11-23 | `release/microsoft-store` | `d5f756277` | test(stt): add paid Reson8 Rumble-2 checks for live, interim, replay, recording and file | `fae3057ae` | test(stt): add paid Reson8 Rumble-2 checks for live, interim, replay, recording and file | 24 ordered picks c5a4ce9fa..d5f756277 -> 669474ff6..fae3057ae; source identical to main except Store What's New; docs f2962637e/1ba238653 and TESTING/CLI-TTS/code-notes hunks excluded; main tests passed, no Store-side build/tests |
| 2026-11-18 | `release/microsoft-store` | `c54d505ae` | feat(onboarding): guide remote setup toward more providers and allow going back | `e6b115a1b` | feat(onboarding): guide remote setup toward more providers and allow going back | Clean pick; changed onboarding files identical to main; main TypeScript passed, no Store-side build/tests |
| 2026-11-18 | `release/microsoft-store` | `fb5e35401` | feat(autostart): disable administrator autostart in Microsoft Store installs and move it to a developer settings block | `904887d99` | feat(autostart): disable administrator autostart in Microsoft Store installs and move it to a developer settings block | Clean pick; source and bindings identical to main; main-only dev launcher fix e82c95e05 excluded (script absent on Store); main cargo check and TypeScript passed, no Store-side build/tests |
| 2026-11-18 | `release/microsoft-store` | `c696e53ea` | fix(autostart): make administrator autostart reliable and repairable | `4b736f86c` | fix(autostart): make administrator autostart reliable and repairable | Clean pick; source identical to main; bindings.ts not regenerated on either branch (new commands called via invoke); main cargo check and a temporary probe test passed, no Store-side build/tests |
| 2026-11-17 | `release/microsoft-store` | `9076a0641` | chore: bump version to 1.0.55 | `986c46259` | chore: bump version to 1.0.55 | 6 ordered runtime/UI picks (bindings bd589b54, TTS hotkey bd49f4be, Administrator title 0bd5c5b33 and hide option 4bf0db8f, ElevenLabs Scribe options 8cc0c643/808545e2) -> c8d85df2b..073504d9f; clean picks, source identical to main; docs, README and main-only publish.yml change dbb444cb excluded; own lock changed app version only; no Store-side build/tests (main build passed per user) |
| 2026-11-15 | `release/microsoft-store` | `213eb619b` | chore: bump version to 1.0.54 | `437270f6e` | chore: bump version to 1.0.54 | Shared application source through 5569623a2 -> merge 4e7d5d23b; compile fixes 42dac5102 -> 9a6238b8d; immutable standard/store marker, Store MSI/ZIP/MSIX updater and portable-storage lock. Build, AVX2, dependencies, signing, workflows and docs stay branch-local; lockfiles resolved independently (app version only). Main Rust 1371/0/27 ignored, frontend 418/0 and TypeScript passed; Store frontend 418/0 and TypeScript passed; Store Rust 1371/0/27 ignored passed after one transient cache file-lock retry. |
| 2026-11-14 | `release/microsoft-store` | `838341c80` | fix(ci): pin Rust 1.99.1 for ONNX Runtime build compatibility | `726b162f9` | fix(linux): qualify the overlay display helper | 1.0.53: 38 ordered code/version adaptations through 9c1890a14 -> 0580763b0; main-only docs, workflows, Nix, updater hunks and tests excluded; branch-specific bindings and independent TLS lock refresh; Store text tests aligned in 1ae839779; Linux helper 5975e19e6 -> 726b162f9; shared CI pin remains main-only; updater 2.10.0, AVX2 and unsigned packaging preserved. Frontend 409/303 and typechecks passed; Store library run 1150 passed, 2 stale tests then fixed (text 42/42); package CI rerun pending. |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
