# Testing

Windows backend tests in this fork must be runnable through the checked-in harness before new tests are kept in the tree.

## Requirements

- Minimum free disk space for tests: `50 GB`.
- No active `cargo`, `tauri`, `rustc`, or `bun` processes when starting a Rust test run.
- Visual Studio 2022 C++ build tools, LLVM/clang, and Rust must be installed.

## Primary Commands

- Run all backend tests:
  `pwsh -NoProfile -File .\test-local.ps1`
- Run library unit tests only:
  `pwsh -NoProfile -File .\test-local.ps1 -LibOnly`
- List available library test names:
  `pwsh -NoProfile -File .\test-local.ps1 -LibOnly -List`
- Run one exact unit test:
  `pwsh -NoProfile -File .\test-local.ps1 -LibOnly -Filter 'plus_overlay_state::tests::test_categorize_auth' -Exact`
- Run a module-focused subset:
  `pwsh -NoProfile -File .\test-local.ps1 -LibOnly -Filter 'language_resolver::tests::'`

## Package Scripts

- Run all backend tests:
  `bun run test:backend`
- Run library unit tests:
  `bun run test:backend:lib`
- List library unit tests:
  `bun run test:backend:list`

## Harness Notes

- `test-local.ps1` imports the same MSVC environment setup used for local builds.
- The harness configures `BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc` and `LIBCLANG_PATH` so `whisper-rs-sys` can build under `cargo test`.
- The harness uses a short `CARGO_TARGET_DIR` to reduce Windows path-length pain.
- By policy, every new test batch must be documented in this file immediately after it is added.

## Windows TTS Focused Tests

The `managers::windows_tts::tests` library subset covers installed-voice
catalog normalization and stable-ID selection, permanent/transient error
classification, WinRT cancellation signaling, strict WAV validation,
mono/stereo downmixing, normalized-duration bounds, common/coprime sample-rate
conversion at exact ratio-chunk boundaries, bounded resampler draining, and
pre-decode cancellation. A Windows-only regression test also queries the
installed-voice catalog twice on one reused thread so WinRT apartment teardown
cannot invalidate the second `SpeechSynthesizer::DefaultVoice()` call.

Run it with:

`pwsh -NoProfile -File .\test-local.ps1 -LibOnly -Filter 'managers::windows_tts::tests'`

On the current development machine the test binary compiles, but the existing
project-wide native test-loader issue terminates it before the Rust harness
starts with `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`. Compile-only validation
remains available with the same harness plus `-NoRun`.

## Playwright + Tauri

- Working Playwright/Tauri instructions live in [[PLAYWRIGHT_TAURI_CONNECTION]].
- Checked-in launcher for agents or no-profile shells:
  `pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1`
- After launch, verify CDP with:
  `Invoke-WebRequest -UseBasicParsing http://127.0.0.1:9333/json/version | Select-Object -ExpandProperty Content`

## Documented Backend Test Areas

Update this section every time new tests are added.

- `src-tauri/src/language_resolver.rs`
  Soniox language code normalization, support checks, hint-list cleanup, and requested-language resolution.
- `src-tauri/src/shortcut_handy_keys.rs`
  HandyKeys modifier-string and alias normalization helpers.
- `src-tauri/src/plus_overlay_state.rs`
  Error categorization, status extraction, display code generation, and envelope defaults.
- `src-tauri/src/clipboard.rs`
  Auto-submit gating and clipboard text normalization helpers.
- `src-tauri/src/cli.rs` and `src-tauri/src/cli_file_conversion.rs`
  First-class TTS file-conversion parsing, comprehensive temporary overrides,
  provider/argument compatibility errors, strict scalar ranges, one-off
  replacement-rule files, Windows default-voice selection, and proof that the
  saved settings snapshot is not mutated. Black-box coverage also verifies that
  `--tts-history true` bypasses only the saved passive-capture toggle.
- `src-tauri/src/managers/history.rs`
  Latest-entry selection rules for mixed transcribe and AI Replace history rows.
- `src-tauri/src/managers/model.rs`
  SHA256 computation and download-verification cleanup/error behavior.
- `src-tauri/src/managers/tts.rs`
  Unicode-safe/lossless semantic chunking, including the regression that a
  leading paragraph newline cannot become a whitespace-only provider request.
- `src-tauri/src/tray.rs`
  Tray helper selection parsing, icon-path mapping, tooltip labeling, and transcript text fallback rules.
- `src-tauri/src/subtitle.rs`
  Subtitle timestamp formatting, whitespace trimming, and file-extension mapping helpers.
- `src-tauri/src/url_security.rs`
  Remote STT preset inference, URL validation, insecure-HTTP guardrails, and canonical LLM base-URL resolution helpers.
- `src-tauri/src/text_replacement_decapitalize.rs`
  Decapitalize trigger state-machine behavior, monitor windows, indicator state, and chunk-transformation helpers.
