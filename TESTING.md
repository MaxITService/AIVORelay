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
- The harness configures `BINDGEN_EXTRA_CLANG_ARGS_x86_64_pc_windows_msvc` and `LIBCLANG_PATH` for native Cargo dependencies under `cargo test`.
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

## Playwright + Tauri

- Working Playwright/Tauri instructions live in [[PLAYWRIGHT_TAURI_CONNECTION]].
- Checked-in launcher for agents or no-profile shells:
  `pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1`
- After launch, verify CDP with:
  `Invoke-WebRequest -UseBasicParsing http://127.0.0.1:9333/json/version | Select-Object -ExpandProperty Content`

## Documented Backend Test Areas

Update this section every time new tests are added.

### Soniox async failure details (2026-08-03)

- `managers::soniox_stt::tests::async_failure_preserves_provider_error_details`
  covers the documented Soniox `error` status payload and verifies that both
  `error_type` and `error_message` remain available in the user-facing error.
- The focused harness filter passed 5/5 tests. The unit tests do not make
  network requests.
- A CLI smoke converted a short existing WAV with the saved Soniox credential
  in 4.38 seconds; it produced 12 words and 76 characters. The CLI correctly
  selected `stt-async-v5` for file transcription when the saved live model was
  `stt-rt-v5`.

### OpenAI realtime transcription payloads (2026-08-03)

- `managers::openai_realtime_whisper::tests::live_transcribe_session_uses_plural_languages_and_prompt`
  also verifies the documented model split: `delay` is emitted for
  `gpt-realtime-whisper` but omitted for `gpt-live-transcribe`, which does not
  support that field.
- The focused harness filter passed 5/5 tests and only inspects generated JSON;
  it does not contact OpenAI.

The existing provider filters were rerun in the same pass:
`managers::remote_stt::tests` (13) and `managers::deepgram_stt::tests` (1),
for 24/24 focused STT tests passing in total.

### TTS settings transparency pass (2026-07-31)

- The active `bun tauri dev` watcher rebuilt and relaunched the debug app after
  the local-install consent command and status-schema changes.
- `bun x tsc --noEmit`, a scoped ESLint run over the changed TTS/settings
  frontend, and `bun src/lib/tts/ttsProviderMetadata.test.ts` passed.
- The metadata test proves every supported provider has complete HTTPS
  human-facing documentation links (never `llms.txt`), a non-empty model
  default, an in-range speed default, unique provider/language values, and
  source/license/size metadata for both downloadable local engines.
- A real debug-CLI status smoke passed for Qwen and Kokoro without synthesis or
  network requests. It exposed the exact managed paths and conservative current
  disk-use estimates: Qwen reported 9,056,372,390 bytes in 7.4 seconds for an
  older installation needing notice repair; Kokoro reported 398,674,114 bytes
  in 2.7 seconds and was ready with its original local `model/LICENSE` present.
  The estimator deduplicates hard-linked files of at least 1 MiB by Windows file
  identity and logically counts smaller files, so it cannot understate use due
  to an unverified small hard link. The older tree's 12,062,403,280-byte logical
  total raised Qwen's conservative install and disk-preflight allowance from
  8 GiB to 16 GiB.
- A live UI smoke confirmed the grouped searchable provider picker, collapsible
  provider help with human documentation links, exact Qwen source/revision/path,
  pre-install size/license disclosure, two separate unchecked consent boxes,
  and a disabled install button. No model download or paid provider request was
  started, and the original Soniox selection was restored.
- The documented Rust build environment passed `cargo check`. The focused `tts`
  filter passed 114 tests and the `local_kokoro` filter passed 8 tests, for
  122/122 passing checks. Coverage includes documented provider defaults, both
  consent flags, source/license/path metadata, settings migration, recursive
  footprint counting, and large-hard-link deduplication.
- The debug binding exporter now removes generator-produced trailing spaces
  while preserving LF/CRLF endings. Its regression test passed, the real hidden
  debug app regenerated `src/bindings.ts`, `bun x tsc --noEmit` passed against
  that output, and the full `git diff --check` passed.

The TTS entries below document source-level unit coverage that was added on
2026-07-31. On the same date, the following focused library subsets passed:
`managers::tts::tests` (26), `managers::local_tts::tests` (7),
`managers::local_kokoro::tests` (6), `managers::tts_llm::tests` (5),
`commands::tts::tests` (11), and `cli_file_conversion::tests` (14), for 69
passing tests in total. `cargo fmt --manifest-path src-tauri/Cargo.toml --all
-- --check`, `bun x tsc --noEmit`, and the documented wrapped `cargo check`
also passed. This is not evidence that mocked HTTP, real watcher events,
cancellation publication, or end-to-end CLI/GUI workflows have passed.

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
  Unit seams for shared cloud PCM response decoding, provider dispatch labels,
  local/system instruction inactivity, operation-ID cancellation/busy-lock
  primitives, Unicode-safe/lossless semantic chunking, in-memory watcher-path
  deduplication, filesystem containment, and disk-capacity thresholds, plus the
  regression that a leading paragraph newline cannot become a whitespace-only
  provider request.
- `src-tauri/src/managers/local_tts.rs` and
  `src-tauri/src/managers/local_kokoro.rs`
  Unit seams for pinned install-manifest validation, disk-reserve preflight
  arithmetic, shared worker WAV validation, local-runtime retry
  classification, and Kokoro voice/language and archive-safety checks. These
  tests do not install models or launch inference workers.
- `src-tauri/src/managers/provider_error.rs`
  Bounded provider error extraction and secret-safe message shaping.
- `src-tauri/src/managers/tts_llm.rs`
  TTS AI-cleanup chunk ordering, retry classification/backoff bounds, and
  resolved-key redaction for provider errors.
- `src-tauri/src/cli_file_conversion.rs`
  Unit-level UTF-8/Cyrillic inline-instruction preservation, BOM-aware large
  instruction-file loading, and proof that the file contents are not copied
  into the parsed CLI argument object. This is not a spawned-process command
  line or full CLI conversion test.
- `src-tauri/src/tray.rs`
  Tray helper selection parsing, icon-path mapping, tooltip labeling, and transcript text fallback rules.
- `src-tauri/src/subtitle.rs`
  Subtitle timestamp formatting, whitespace trimming, and file-extension mapping helpers.
- `src-tauri/src/url_security.rs`
  Remote STT preset inference, URL validation, insecure-HTTP guardrails, and canonical LLM base-URL resolution helpers.
- `src-tauri/src/text_replacement_decapitalize.rs`
  Decapitalize trigger state-machine behavior, monitor windows, indicator state, and chunk-transformation helpers.
