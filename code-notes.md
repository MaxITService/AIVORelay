# Fork Code Notes

Files that differentiate this fork from the original [cjpais/Handy](https://github.com/cjpais/Handy).

## Verified UI Issues Snapshot (February 19, 2026)

Only reproducible, screen-level findings are listed here (detailed repro steps are in `plans/screen-bug-audit.md`):

- `Voice Commands`
  - `VC-001`: Fuzzy matching settings are not persisted because several `updateSetting(...)` keys are missing `settingUpdaters` wiring in `settingsStore`.
  - `VC-002`: Voice Commands “Refresh models” currently calls `fetchLlmModels("post_processing")`, so when Voice Commands provider differs from Post-Processing provider, refresh targets the wrong provider bucket.
- `Debug`
  - `DBG-002`: Switching shortcut engine to `Tauri` clears incompatible bindings immediately in backend, before user confirms restart; canceling restart does not restore cleared bindings.
  - `DBG-003`: Turning off experimental `Voice Commands` in Debug hides the menu item, but does not disable runtime `voice_command_enabled`; shortcut execution can remain active.
- `Connector`
  - `CON-001`: Port change can report success before actual bind health is known (bind failure is detected asynchronously), so inline port error handling may not trigger immediately.

Sidebar screens reviewed without confirmed reproducible breakages in this cycle:
- `General / Speech / Mic`, `Models`, `Advanced`, `LLM Post Processing`, `AI Replace`, `Transcribe File`, `Text Processing`, `Speech Processing`, `User Interface`, `History`, `About`.

Reference audit log:
- `plans/screen-bug-audit.md`

## Recent Convergence (Fork-Safe)

These changes move selected areas toward upstream behavior without removing fork-only features:

- Soniox context support for profiles/default:
  - Added Soniox context fields (`context.general` JSON, `context.text`, `context.terms`) to global/default settings and transcription profiles.
  - `TranscriptionProfiles` now shows Soniox-specific collapsible context editors when Soniox provider is active, while hiding non-Soniox STT prompt UI.
  - Soniox live/non-live/file request payloads now include validated context when present.
- `scripts/check-translations.ts`
  - Translation consistency checker used in CI to detect missing/extra locale keys.
- `.github/workflows/lint.yml`
  - Runs `bun run check:translations` before lint.
- `src/components/settings/models/ModelsSettings.tsx`
  - Additive Models management page in Settings (download/select/cancel/delete).
  - Added custom-model help text and custom badges.
- `src/components/Sidebar.tsx`
  - Added a Models navigation section (reuses existing translation keys).
- `src-tauri/src/managers/model.rs`
  - `ModelInfo` now includes `supports_translation`, `is_recommended`, `supported_languages`.
  - Added `is_custom` and startup auto-discovery of user-provided Whisper `.bin` models from the models folder.
  - Custom model deletion now removes the in-memory entry immediately.
  - Added `EngineType::SenseVoice` and local model entries `sense-voice-int8` + `breeze-asr`.
  - Emits `model-deleted` event after successful deletion.
- `src-tauri/src/commands/models.rs`
  - Deleting active model now unloads it and clears selected model safely.
  - Compatibility command `get_recommended_first_model` now consults model metadata first.

## Whitespace Policy Refactor (Text Processing)

- Settings navigation and page title are now **Text Processing**.
- Legacy debug toggle for trailing-space append is no longer used; whitespace behavior lives in Text Processing.
- Output whitespace behavior is defined by explicit policy modes in settings schema:
  - `output_whitespace_leading_mode` (`preserve` | `remove_if_present` | `add_if_missing`)
  - `output_whitespace_trailing_mode` (`preserve` | `remove_if_present` | `add_if_missing`)
- Text Processing UI exposes 4 whitespace controls as 2 mutually-exclusive pairs:
  - leading: remove vs add
  - trailing: remove vs add
- Backend uses centralized helper `apply_output_whitespace_policy(...)` for final output normalization.
- The same whitespace policy is applied consistently across local, remote, Soniox, and file-transcription text output paths.
- Soniox streaming applies leading policy on first emitted chunk and trailing policy at finalization.

Touched files in this refactor:

- `src/components/settings/debug/DebugSettings.tsx`
- `src/components/settings/text-replacement/TextReplacementSettings.tsx`
- `src/i18n/locales/en/translation.json`
- `src/i18n/locales/ru/translation.json`
- `src/stores/settingsStore.ts`
- `src-tauri/src/settings.rs`
- `src-tauri/src/shortcut.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/actions.rs`
- `src-tauri/src/clipboard.rs`
- `src-tauri/src/managers/transcription.rs`
- `src-tauri/src/managers/soniox_stt.rs`
- `src-tauri/src/commands/file_transcription.rs`
- `src-tauri/src/soniox_stream_processor.rs`
- `src/bindings.ts`

## New Files (Fork-Specific)

### Backend (Rust)

| File                                           | Purpose                                                                                                                                                                                                                                                                                                                                                    |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/managers/connector.rs`          | **Main connector module**: HTTP server (port 38243) for extension communication. Extension polls `GET /messages` with Bearer auth, AivoRelay returns `{cursor, messages[], config, passwordUpdate?}`. Handles text messages, bundle (with image attachments via `/blob/*`), and keepalive messages. **Includes two-phase password rotation** for security. |
| `src-tauri/src/commands/connector.rs`          | Tauri commands for connector: `connector_get_status`, `connector_is_online`, `connector_start_server`, `connector_stop_server`, `connector_queue_message`, `connector_cancel_message`.                                                                                                                                                                       |
| `src-tauri/src/managers/remote_stt.rs`         | Remote Speech-to-Text manager. Handles OpenAI-compatible API calls, WAV encoding, API key storage (Windows Credential Manager), debug logging.                                                                                                                                                                                                             |
| `src-tauri/src/commands/remote_stt.rs`         | Tauri commands exposing remote transcription functionality to frontend: OpenAI-compatible commands plus Soniox API-key commands (`soniox_has_api_key`, `soniox_set_api_key`, `soniox_clear_api_key`).                                                                                                                                                       |
| `src-tauri/src/managers/soniox_stt.rs`         | Soniox non-live/async transcription manager: request payload construction, cancellation tracking, language handling, and async file transcription calls. Includes async create-transcription options (`language_hints`, `enable_speaker_diarization`, `enable_language_identification`) for file jobs.                                                                                                                                                                                                    |
| `src-tauri/src/managers/soniox_realtime.rs`    | Soniox live WebSocket manager: streaming audio frames, keepalive/finalize control, endpoint/language/speaker options, and final chunk collection.                                                                                                                                                                                                           |
| `src-tauri/src/secure_keys.rs`                 | **Secure API key storage** (Windows only): Unified interface for storing all LLM API keys (Remote STT, Post-Processing, AI Replace) in Windows Credential Manager. Includes migration logic from JSON settings.                                                                                                                                            |
| `src-tauri/src/plus_overlay_state.rs`          | Extended overlay states for Remote STT error display. Categorizes errors (TLS, timeout, network, server), emits typed payloads to overlay, auto-hides after 3s.                                                                                                                                                                                            |
| `src-tauri/src/region_capture.rs`              | **Native region capture** (Windows only): Captures all monitors into single canvas, opens full-screen overlay for region selection with resize handles. Returns cropped PNG bytes directly to connector without disk I/O.                                                                                                                                  |
| `src-tauri/src/commands/region_capture.rs`     | Tauri commands for region capture overlay: `region_capture_confirm`, `region_capture_cancel`.                                                                                                                                                                                                                                                              |
| `src-tauri/src/commands/voice_command.rs`      | **Voice Command Center** (Windows only): Tauri command `execute_voice_command` runs approved PowerShell commands after user confirmation. Includes safety validation, non-blocking execution for silent commands, and support for `pwsh.exe` and `wt.exe`. |
| `src-tauri/src/commands/file_transcription.rs` | **File Transcription**: Handles logic for transcribing audio files. Decodes various audio formats (wav, mp3, etc.), manages output formats, coordinates with local/remote providers, and supports Soniox async overrides (language hints, speaker diarization, language identification).                                                                    |
| `src-tauri/src/subtitle.rs`                    | **Subtitle Formatting**: Logic for generating timestamped subtitles (SRT/VTT). Used by `file_transcription.rs` to structure transcription segments into standard subtitle formats.                                                                                                                                                                          |
| `src-tauri/src/audio_toolkit/text.rs`          | **Text Post-Processing**: Logic for cleaning up transcriptions, including collapsing repeated 1-2 letter stutters (e.g., "I-I" → "I") and filtering filler words ("uhm", "uh").                                                                                                                                                                             |
| `src-tauri/src/input_source.rs`                | **OS Language Detection**: Utilities to detect the current system input language, used for automatic language switching in transcription profiles.                                                                                                                                                                                                         |
| `src-tauri/src/active_app.rs`                  | **Active App Context**: Captures frontmost window title (Windows) for LLM template variables like `${current_app}`.                                                                                                                                                                                                                                         |
| `src-tauri/src/transcript_context.rs`          | **Prompt Context Cache**: Stores short per-app transcript history with expiry, used for `${short_prev_transcript}`.                                                                                                                                                                                                                                          |
| `src-tauri/src/managers/key_listener.rs`       | **rdev Key Listener** (Windows): Low-level keyboard hook using rdev library. Tracks modifier state, parses shortcut strings (e.g., "ctrl+shift+a", "caps lock"), emits `rdev-shortcut` events. Supports keys that Tauri can't handle: CapsLock, NumLock, ScrollLock, Pause, modifier-only shortcuts.                                                       |
| `src-tauri/src/commands/key_listener.rs`       | Tauri commands for key listener lifecycle + shortcuts: `key_listener_start`, `key_listener_stop`, `key_listener_is_running`, `key_listener_get_modifiers`, `key_listener_register_shortcut`, `key_listener_unregister_shortcut`, `key_listener_is_shortcut_registered`, `key_listener_get_registered_shortcuts`.                                                                                 |
| `src-tauri/src/language_resolver.rs`           | **Soniox language compatibility resolver**: Canonicalizes profile/default/OS language values, maps locale variants to Soniox ISO base codes, validates support, and normalizes Soniox hint lists with safe fallback behavior.                                                                                                                           |
| `src-tauri/src/text_replacement_decapitalize.rs` | Decapitalize trigger state machine for manual edits: one-shot timeout trigger + standard STT post-stop monitoring window, with separate handling for realtime chunk flow vs. standard final-output flow and Unicode-safe first-letter lowercasing. |

### Frontend (React/TypeScript)

| File                                                                   | Purpose                                                                                                                                                                 |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/settings/remote-stt/RemoteSttSettings.tsx`             | UI for Remote STT configuration: base URL, model ID, API key management, connection testing, debug log viewer.                                                          |
| `src/components/settings/SonioxContextEditor.tsx`                       | Reusable Soniox context editor used in transcription profile cards (default/custom/new): collapsible `context.general` JSON, `context.text`, `context.terms` inputs with inline validation and help/examples. |
| `src/components/settings/ai-replace/AiReplaceSelectionSettings.tsx`    | Main AI Replace settings page in sidebar: shortcut/push-to-talk, no-selection mode, quick-tap controls, prompt fields, and AI Replace-specific LLM provider/model config. |
| `src/components/settings/advanced/AiReplaceSettings.tsx`               | Legacy AI Replace settings component kept in tree (not mounted in current sidebar flow).                                                                               |
| `src/components/settings/browser-connector/ConnectorStatus.tsx`        | Extension status indicator component showing online/offline status with "last seen" time when offline.                                                                  |
| `src/components/icons/SendingIcon.tsx`                                 | Monochrome SVG icon (upload arrow) for "sending" overlay state. Matches pink style (`#FAA2CA`) of other icons.                                                          |
| `src/overlay/plus_overlay_states.ts`                                   | TypeScript types for extended overlay states (`error`, `sending`). Error category enum and display text mapping.                                                        |
| `src/region-capture/RegionCaptureOverlay.tsx`                          | React component for native region selection: state machine (idle→creating→selected), mouse handling, resize handles.                                                    |
| `src/region-capture/RegionCaptureOverlay.css`                          | Styles for region capture overlay: dim areas, selection border, resize handles, cursor states.                                                                          |
| `src/command-confirm/CommandConfirmOverlay.tsx`                        | **Voice Command Center**: Confirmation popup showing suggested PowerShell command with Run/Edit/Cancel buttons.                                                         |
| `src/command-confirm/CommandConfirmOverlay.css`                        | Styles for command confirmation overlay: glassmorphism, dark theme, vibrant accent colors.                                                                              |
| `src/soniox-live-preview/SonioxLivePreview.tsx`                        | **Soniox Live Preview Window**: Separate visual-only window that renders Soniox live final + interim text updates without affecting paste behavior, with live appearance updates (theme/opacity/colors) and separate final/interim font-color rendering. Supports interactive actions and hotkey-driven visibility. |
| `src/lib/utils/previewHotkeys.ts`                                     | **Preview Hotkeys Core**: Centralized logic for managing and executing hotkeys assigned to the live preview window. Handles global shortcut registration and action routing. |
| `src/components/ui/HotkeyCapture.tsx`                                  | **Hotkey Capture UI**: Reusable component for capturing and validating keyboard shortcuts for the live preview feature. |
| `src/soniox-live-preview/SonioxLivePreview.css`                        | Styles for Soniox live preview window (always-on-top visual stream panel), now driven by CSS variables from app settings.                                              |
| `src/components/settings/voice-commands/VoiceCommandSettings.tsx`      | Settings UI for managing predefined voice commands, similarity thresholds, and LLM fallback toggle.                                                                     |
| `src/components/settings/transcribe-file/TranscribeFileSettings.tsx`   | UI for "Transcribe Audio File" feature: Drag-and-drop zone, file info, output format selection (Text/SRT/VTT), optional model override, and results display. Includes Soniox-only per-run options (language hints + recognition flags) and async-model auto-switch notice. |
| `src/components/settings/text-replacement/TextReplacementSettings.tsx` | UI for "Text Replacement" feature: Add/remove replacement rules with enable/disable toggles. Supports escape sequences for special characters (\\n, \\r\\n, \\t, \\\\), regex matching, and adjustable execution order (Before/After LLM). Also includes "Decapitalize After Manual Edit" controls (enable toggle, monitored key capture, combo-presence behavior hint, timeout, standard STT post-stop monitor window, tell-me-more help) plus non-blocking conflict warnings when monitored key overlaps other shortcuts. |
| `src/components/settings/audio-processing/AudioProcessingSettings.tsx` | UI for audio processing settings: VAD sensitivity, stutter collapsing, and transcription cleaning options.                                                                                                                                                                                           |
| `src/components/settings/debug/ShortcutEngineSelector.tsx`             | **Shortcut Engine Selector** (Windows): UI for switching between Tauri (high-perf, limited keys) and rdev (all keys, higher CPU) engines. Shows incompatible shortcuts warning, requires app restart. Located in Debug → Experimental Features.                                                     |
| `src/stores/transcribeFileStore.ts`                                    | Session store for Transcribe File UI state (selected file, output mode, profile selection, results).                                                                    |
| `src/lib/constants/sonioxLanguages.ts`                                 | Shared Soniox language utilities: supported language set, code normalization (`xx-YY`/`xx_YY` to base ISO where needed), support checks, and hint parsing helpers used by settings/profile UI.                                                                                                       |

### Development & Build Tools

| File                    | Purpose                                                                                                                                                                                                                                                                                   |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `build-local.ps1`       | **Local Build Script** (Windows): Automates local unsigned builds without code signing. Sets up VS environment via Launch-VsDevShell.ps1, checks for Vulkan SDK, verifies tools, installs dependencies, and builds release/debug MSI. Equivalent to GitHub Actions build without signing. |
| `build-unsigned.js`     | **Unsigned Build Helper**: Node.js script that cleans old artifacts and runs `tauri build --no-sign` with updater disabled. Called by `bun run build:unsigned` and by build-local.ps1.                                                                                                   |

## Modified Files

### Backend Core Logic

| File                         | Changes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/actions.rs`   | Added new shortcut actions: `AiReplaceSelectionAction`, `SendToExtensionAction`, `SendToExtensionWithSelectionAction`, `SendScreenshotToExtensionAction`. These handle the new voice-to-LLM, connector, and screenshot workflows. **Uses `show_sending_overlay()` for Remote STT instead of `show_transcribing_overlay()`.** Also adds reusable LLM template variable resolution for prompts (`${current_app}`, `${short_prev_transcript}`, `${language}`, `${profile_name}`, `${time_local}`, `${date_iso}`, `${translate_to_english}`, `${selection}`). Includes Soniox language resolution for live mode so profile/default/OS language values are normalized and unsupported values fall back safely. Local-language resolution for local models now treats Whisper and SenseVoice as language-selectable engines. Applies optional decapitalization trigger after manual edit key detection, with dedicated post-stop monitor arming for standard STT (non-realtime) only. |
| `src-tauri/src/overlay.rs`   | Added `show_sending_overlay()` function, Soniox live finalization overlay helpers, and made `force_overlay_topmost()` public for reuse. Also includes Soniox live preview window lifecycle helpers (create/show/hide/reset/update) with settings-driven enable/position/size, dynamic near-cursor positioning, custom X/Y and custom pixel size support, plus appearance payload/events (theme/opacity/final+interim font colors/accent/interim opacity). Added a command to open a resizable demo preview window with sample text for visual tuning.                                                                                                   |
| `src-tauri/src/settings.rs`  | Extended `AppSettings` with: `transcription_provider`, `remote_stt` settings, **Soniox settings** (`soniox_model`, live toggle, language hints, strict/profile hint mode, endpoint/language-id/speaker options, keepalive/finalize timing), Soniox live preview appearance/position fields (theme/opacity/font/accent/interim opacity, dynamic near-cursor distance), `ai_replace_*` fields, `connector_*` fields (including `connector_password` for auth), `screenshot_*` fields, individual push-to-talk settings, `shortcut_engine` (Windows). Added `RemoteSttSettings`, `TranscriptionProvider`, `ShortcutEngine` enums. Added explicit `store.save()` in `write_settings()` to prevent race conditions on restart. Includes safety controls for transcript context variable caching (`llm_context_prev_transcript_*`). |
| `src-tauri/src/lib.rs`       | Registered new managers (`RemoteSttManager`, `ConnectorManager`, `SonioxSttManager`, `SonioxRealtimeManager`) and commands including individual push-to-talk commands and screenshot settings commands. Starts connector server on app init. Handles tray icon creation and event loop. Also registers the shared `language_resolver` module used by Soniox paths. Creates overlay windows at startup, including Soniox live preview. |
| `src-tauri/src/shortcut.rs`  | Added shortcut bindings for new actions (AI Replace, Send to Extension, Send Screenshot to Extension). Added commands for individual push-to-talk settings and screenshot settings, plus logic to use per-binding push-to-talk instead of global setting for fork-specific actions. Integrated OS language detection for automatic profile switching. **Added dual-engine support (Windows)**: conditionally starts rdev listener, routes shortcuts to Tauri or rdev based on compatibility, clears incompatible bindings on engine switch. Soniox language-hint settings are validated/normalized before saving. Also manages passive monitored-key registration for text-replacement decapitalization trigger, exposes setting command for standard STT post-stop monitor window timeout, and now handles Soniox live preview appearance/position/size commands including custom X/Y, custom pixel width/height, and separate interim font color. |
| `src-tauri/src/clipboard.rs` | Enhanced clipboard handling for AI Replace selection capture.                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/input.rs`     | Added selection capture utilities for Windows.                                                                                                                                                                                                                                                                                                                                       |
| `src-tauri/src/tray.rs`      | Custom tray menu implementation: added "Copy Last Transcript" action and access to quick settings.                                                                                                                                                                                                                                                                                |

### Backend Support

| File                                         | Changes                                                                                                                                                           |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/commands/mod.rs`              | Exports command modules including remote STT and file transcription paths used by Soniox provider features.                                                        |
| `src-tauri/src/managers/mod.rs`              | Exports manager modules including `remote_stt`, `soniox_stt`, `soniox_realtime`, and `connector`.                                                                 |
| `src-tauri/src/audio_toolkit/mod.rs`         | Added `encode_wav_bytes()` for Remote STT API.                                                                                                                    |
| `src-tauri/src/audio_toolkit/audio/utils.rs` | WAV encoding utilities.                                                                                                                                           |
| `src-tauri/src/audio_toolkit/audio/recorder.rs` | Mic error handling logic: detects and reports when the microphone is unavailable or used by another process. |
| `src-tauri/src/managers/transcription.rs` | Local engine runtime now includes SenseVoice (load/unload + transcription paths for regular, override, and segment-returning calls). |
| `src-tauri/src/commands/file_transcription.rs` | Soniox async integration for file transcription: provider routing, **latest-only model enforcement (`stt-async-v4`)** for Soniox file jobs, optional Soniox overrides, and informational `info_message` payload for UI display when auto-switching. |
| `src-tauri/src/managers/soniox_stt.rs`       | Centralized Soniox language handling via resolver: normalizes requested language and falls back to auto when unsupported/invalid for non-live transcription. |
| `src-tauri/src/managers/soniox_realtime.rs`  | Live-session language hints are normalized/validated via resolver before request payload creation. Emits visual Soniox live preview updates (final + interim) and manages preview window visibility during session start/finish/cancel. |
| `src-tauri/src/utils.rs`                     | Central cancellation path now also cancels Soniox live/non-live operations to avoid orphaned requests after user stop/cancel. |
| `src-tauri/Cargo.toml`                       | Added dependencies/features: `keyring` (credential storage), `reqwest` features, `axum` + `tower-http` (HTTP server/CORS for connector), `notify` (file system watching for screenshots), `windows` crates for input language detection, and `transcribe-rs = 0.2.5` with `sense_voice` feature for local STT. |
| `src-tauri/resources/default_settings.json`  | Default values for new settings, including Soniox defaults (model, hints, strict/profile-hint options, live behavior toggles). |

### Frontend Settings UI

| File                                                                     | Changes                                                                                              |
| ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `src/components/icons/index.ts`                                          | Exports `SendingIcon` component.                                                                     |
| `src/components/settings/advanced/AdvancedSettings.tsx`                  | Advanced settings group for startup/paste behavior: Start Hidden, Autostart, Paste Method, Clipboard Handling, Auto Submit, and Model Unload Timeout.                                                   |
| `src/components/settings/browser-connector/BrowserConnectorSettings.tsx` | Added extension status indicator section and screenshot settings (capture command, folder, timeout). |
| `src/components/settings/general/GeneralSettings.tsx`                    | Minor adjustments for new settings layout.                                                           |
| `src/components/settings/user-interface/UserInterfaceSettings.tsx`       | Added dedicated Soniox live preview settings group in User Interface with controls for enable/disable, preview demo window button, position (`top/bottom/near_cursor/custom_xy`), cursor distance, custom X/Y and custom width/height in px (both slider + number input), theme, transparency, separate final/interim font colors, accent color, interim opacity, and expandable help blocks. |
| `src/components/Sidebar.tsx`                                             | Navigation for new settings sections.                                                                |
| `src/hooks/useSettings.ts`                                               | Hooks for new settings: `setTranscriptionProvider`, `updateRemoteStt*`, `updateAiReplace*`, and Soniox-related updates via settings store actions.          |
| `src/components/settings/remote-stt/RemoteSttSettings.tsx`              | Soniox hint input now parses/normalizes codes and surfaces warnings for rejected hints.              |
| `src/components/settings/TranscriptionProfiles.tsx`                      | Provider-aware language handling: filters language choices for Soniox-supported codes and warns when stored profile language is unsupported (fallback to auto). Also filters local-language options by the selected local model `supported_languages` while keeping `auto`/`os_input` fallbacks. |
| `src/components/settings/TranscriptionSystemPrompt.tsx`                 | Prompt editor is hidden when Soniox provider is active (Soniox path does not use this per-model system prompt UI). SenseVoice is treated as non-promptable in prompt-capability detection. |
| `src/components/settings/TranslateToEnglish.tsx`                        | Translate-to-English toggle is disabled for Soniox provider and shows provider-specific unsupported description text. |
| `src/lib/constants/languages.ts`                                        | Added `yue` (Cantonese) to language options used in transcription settings and profiles. |
| `src/stores/settingsStore.ts`                                            | State management for new settings including Soniox-specific setter wiring (model, live flags, hints, strictness, endpoint/language-id/speaker flags, timing), plus Soniox live preview controls and appearance updater wiring. |
| `src/i18n/locales/en/translation.json`                                   | English UI strings including Soniox settings/help, hints, live-mode explanations, and validation messages. |
| `src/i18n/locales/ru/translation.json`                                   | Russian localization for Soniox settings/help text and behavior explanations. |
| `src/bindings.ts`                                                        | Auto-generated Tauri command bindings including Soniox commands/types (`RemoteSoniox` provider, Soniox settings commands, file override types). `EngineType` now includes `SenseVoice`. |

### Other Modified

| File                                       | Changes                                                                                                                                                                                |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/App.tsx`                              | Event listeners for new features (remote-stt-error, ai-replace-error, screenshot-error).                                                                                               |
| `src/components/model-selector/ModelSelector.tsx` | Adjusted for provider switching, including Soniox provider behavior in remote/local model selection flow.                                                                                                                      |
| `src/components/onboarding/Onboarding.tsx` | Updated for Remote STT option.                                                                                                                                                         |
| `src/overlay/RecordingOverlay.tsx`         | Extended to handle `error` and `sending` states with categorized error messages. Uses `SendingIcon` for "sending" state. Accepts extended payload object instead of string-only state. |
| `src/overlay/RecordingOverlay.css`         | Added `.error-text` and `.overlay-error` styles for error state display.                                                                                                               |
| `vite.config.ts`                           | Added multi-entry build target for Soniox live preview window (`src/soniox-live-preview/index.html`).                                                                                 |

## Feature → File Mapping

### Remote STT API

```
User configures in UI
    └─► RemoteSttSettings.tsx
            └─► useSettings.ts → settings.rs
                    └─► remote_stt.rs (manager)
                            └─► OpenAI-compatible API
```

### Soniox STT (Live + Async + File)

```
User selects Soniox provider
    └─► settings.rs (TranscriptionProvider::RemoteSoniox + soniox_* settings)
            └─► RemoteSttSettings.tsx / settingsStore.ts (Soniox controls)
                    ├─► actions.rs
                    │     └─► language_resolver.rs (profile/default/OS language normalization)
                    │             └─► soniox_realtime.rs (live WebSocket path)
                    │                     └─► keepalive/finalize + final text
                    └─► soniox_stt.rs (non-live + async file path)
                            └─► file_transcription.rs (SonioxFileTranscriptionOptions overrides)
```

### AI Replace Selection

```
User presses shortcut + speaks instruction
    └─► shortcut.rs → actions.rs (AiReplaceSelectionAction)
            └─► input.rs (capture selection)
            └─► transcription (local or remote)
            └─► llm_client.rs → LLM API
            └─► clipboard.rs (paste result)
```

### Send to Extension (Connector)

```
User presses shortcut + speaks
    └─► shortcut.rs → actions.rs (SendToExtensionAction)
            └─► transcription
            └─► managers/connector.rs → queue_message() or queue_bundle_message()
                    └─► message added to queue with {id, type, text, ts, attachments?}

Extension polls server
    └─► GET http://127.0.0.1:38243/messages?since=<cursor>
            └─► Authorization: Bearer <password>
            └─► Returns {cursor, messages[], config, passwordUpdate?}
    └─► GET /blob/<attId> for image attachments (also requires auth)
```

### Extension Protocol Notes

- **Message types**: `text`, `bundle` (with attachments), `keepalive`
- **Keepalive**: Extension should filter `msg_type === "keepalive"` to avoid pasting "keepalive" into pages
- **Password rotation**: On first connect, server sends `passwordUpdate`; extension must POST `{"type":"password_ack"}` to commit
- **Blob auth**: `/blob/*` endpoint requires Bearer auth (Extension provides this header automatically; it is NOT sent in metadata for security)

### Voice Command Center (NEW)

```
User presses voice_command shortcut + speaks
    └─► shortcut.rs → actions.rs (VoiceCommandAction)
            └─► transcription (local or remote)
            └─► find_matching_command() → fuzzy match against predefined commands
                    │
                    ├─► MATCH FOUND → show_command_confirm_overlay() → User confirms → execute_voice_command()
                    │
                    └─► NO MATCH + LLM fallback enabled
                            └─► generate_command_with_llm() → LLM generates PowerShell one-liner
                                    └─► show_command_confirm_overlay() → User confirms/edits → execute_voice_command()
```

- **Two modes**: Predefined commands (fast, offline) and LLM-generated commands (smart, flexible)
- **Similarity matching**: Configurable threshold (default 0.75) using word-based Jaccard similarity
- **Safety**: Always shows confirmation popup before executing any command

### Transcription Profiles

```
User creates profile in Settings
    └─► TranscriptionProfiles.tsx → commands.addTranscriptionProfile()
            └─► shortcut.rs → creates profile + shortcut binding (transcribe_profile_xxx)
                    └─► settings.rs → TranscriptionProfile {id, name, language, translate, system_prompt}

User presses profile shortcut
    └─► shortcut.rs → ACTION_MAP["transcribe"] (falls back from transcribe_profile_xxx)
            └─► actions.rs → perform_transcription_for_profile()
                    ├─► Uses profile.language + translate_to_english overrides (local STT)
                    └─► Uses profile.system_prompt if set, else global per-model prompt (remote STT)
```

- **System Prompt Limits**: Character limits are enforced based on the STT model (Whisper: 896, Deepgram: 2000)
- **Shared Logic**: Frontend uses `getModelPromptInfo()` from `TranscriptionSystemPrompt.tsx`; backend validates in `remote_stt.rs`

### Transcribe Audio File

```
User drops file in UI
    └─► TranscribeFileSettings.tsx
            └─► commands.transcribeAudioFile()
                    └─► file_transcription.rs (decodes audio)
                    │
                    ├─► transcription (local or remote)
                    │
                    └─► subtitle.rs (formats SRT/VTT if requested)
                            └─► segments_to_srt() / segments_to_vtt()
```

- **Formatting**: Supports Text, SRT, and VTT output.
- **Timestamping**: Accurate timestamps require Local model; Remote STT currently returns text-only (single segment).
- **Audio Processing**: Supports wav, mp3, m4a, ogg, flac, webm. Resamples to 16kHz automatically.
- **Soniox File Mode**: Uses Soniox async endpoint with latest-only model mapping to `stt-async-v4`, accepts per-file options (language hints, diarization, language identification), and surfaces an in-UI informational message when auto-switch is applied.

### Shortcut Engine (Windows)

```
User selects engine in Settings → Debug → Experimental Features
    └─► ShortcutEngineSelector.tsx
            └─► invoke("set_shortcut_engine_setting")
                    └─► shortcut.rs → saves to settings, clears incompatible bindings if switching to Tauri
                            └─► relaunch() required to apply

On app startup
    └─► init_shortcuts() in shortcut.rs
            ├─► If Tauri engine: register via tauri-plugin-global-shortcut (WM_HOTKEY)
            └─► If rdev engine: start key_listener.rs → rdev::listen() (WH_KEYBOARD_LL hook)
                    └─► emits "rdev-shortcut" events → handle_rdev_shortcut_event()
```

- **Tauri engine**: High performance, zero polling, uses Windows `RegisterHotKey` API. Cannot support CapsLock, NumLock, ScrollLock, Pause, or modifier-only shortcuts.
- **rdev engine**: Supports ALL keys via low-level hook. Processes every keystroke system-wide (higher CPU). May trigger antivirus false positives.
- **Default**: Tauri (for performance). Users needing special keys switch to rdev manually.

## Entry Points for Common Tasks

| Task                                | Start Here                                                                               |
| ----------------------------------- | ---------------------------------------------------------------------------------------- |
| Change core transcription flow      | `actions.rs` → `perform_transcription()` helper                                          |
| Change AI Replace behavior          | `actions.rs` → `AiReplaceSelectionAction::stop()` or `ai_replace_with_llm()`             |
| Change message format for Connector | `actions.rs` → `build_extension_message()`                                               |
| Debug recording/mute logic          | `actions.rs` → `prepare_stop_recording()` or `start_recording_with_feedback()`           |
| Add new AI Replace setting          | `settings.rs` → add field, `AiReplaceSelectionSettings.tsx` → add UI                      |
| Change Remote STT API handling      | `managers/remote_stt.rs` → `transcribe()`                                                |
| Change Soniox non-live behavior     | `managers/soniox_stt.rs` → request options, language handling, async/file paths          |
| Change Soniox live behavior         | `managers/soniox_realtime.rs` → WS payload, keepalive/finalize, timeout behavior         |
| Add new shortcut action             | `actions.rs` → impl `ShortcutAction`, register in `ACTION_MAP`                           |
| Change selection capture logic      | `input.rs` (Windows-specific)                                                            |
| Add new Tauri command               | `commands/*.rs` → add fn, `commands/mod.rs` → export                                     |
| Change extension status timeout     | `managers/connector.rs` → `POLL_TIMEOUT_MS` constant                                      |
| Customize status display            | `ConnectorStatus.tsx`                                                                    |
| Change connector password           | `settings.rs` → `connector_password` field, `BrowserConnectorSettings.tsx` → password UI |
| Add/modify transcription profiles   | `settings.rs` → `TranscriptionProfile`, `shortcut.rs` → profile commands                 |
| Change profile system prompt limits | `TranscriptionSystemPrompt.tsx` → `getModelPromptInfo()`, `managers/remote_stt.rs`       |
| Change Soniox language resolution   | `language_resolver.rs` → `resolve_requested_language_for_soniox()` and `normalize_soniox_hint_list()` |
| Change Soniox file options UI/flow  | `TranscribeFileSettings.tsx` + `commands/file_transcription.rs`                           |

## Key Data Structures

| Structure               | File                    | Purpose                                                                          |
| ----------------------- | ----------------------- | -------------------------------------------------------------------------------- |
| `AppSettings`           | `settings.rs`           | All app settings, includes `ai_replace_*`, `remote_stt`, `soniox_*`, `connector_*` |
| `RemoteSttSettings`     | `settings.rs`           | base_url, model_id, debug_mode, debug_capture                                    |
| `TranscriptionProfile`  | `settings.rs`           | Custom shortcut profile: id, name, language, translate_to_english, system_prompt |
| `TranscriptionProvider` | `settings.rs`           | Enum: `Local`, `RemoteOpenAiCompatible`, `RemoteSoniox`                          |
| `EngineType`            | `managers/model.rs`     | Local engine enum used by model metadata/runtime (`Whisper`, `Parakeet`, `Moonshine`, `SenseVoice`) |
| `SonioxLanguageResolution` | `language_resolver.rs` | Result object for profile/default/OS language normalization and fallback status |
| `SonioxHintListNormalization` | `language_resolver.rs` | Normalized Soniox hint list + rejected hints metadata for UI/backend validation |
| `SonioxRealtimeOptions` | `managers/soniox_realtime.rs` | Live Soniox stream options: language hints, strictness, endpoint and metadata toggles |
| `SonioxAsyncTranscriptionOptions` | `managers/soniox_stt.rs` | Async Soniox file-job options for create-transcription payload fields (language hints, speaker diarization, language identification) |
| `SonioxFileTranscriptionOptions` | `commands/file_transcription.rs` | Per-file Soniox overrides for language hints, speaker diarization, language identification |
| `ShortcutAction` trait  | `actions.rs`            | Interface for all shortcut actions (start/stop)                                  |
| `ACTION_MAP`            | `actions.rs`            | Registry of all available shortcut actions                                       |
| `ConnectorManager`      | `managers/connector.rs` | HTTP server tracking extension status via polling                                |
| `ConnectorStatus`       | `managers/connector.rs` | Status struct with `status`, `last_poll_at`, `server_running`, `port`, `server_error` fields |

## Change Impact

| If you change...         | Check also...                                                              |
| ------------------------ | -------------------------------------------------------------------------- |
| `AppSettings` fields     | `src-tauri/resources/default_settings.json`, `src/hooks/useSettings.ts`, `src/stores/settingsStore.ts` |
| Tauri commands           | Run `bun run tauri dev` to regenerate `bindings.ts`                        |
| Remote STT API format    | `encode_wav_bytes()` in audio_toolkit                                      |
| Local model engine/language behavior | `managers/model.rs`, `managers/transcription.rs`, `actions.rs`, `TranscriptionProfiles.tsx`, `TranscriptionSystemPrompt.tsx` |
| Connector message format | Extension expects `{id, type, text, ts, attachments?}` from polling server |
| Connector auth           | Extension uses `Authorization: Bearer <password>` header                   |
| Prompt templates         | Variables: `${instruction}` (voice), `${output}` (selected/input text)     |
| Quick Tap (AI Replace)   | Skips STT if < 800ms; uses `ai_replace_quick_tap_system_prompt`            |
| Allow No Voice           | If enabled, sends `${output}` only with specific "No Voice" system prompt  |

### Footer VRAM Meter (Microsoft Store Edition)

- New backend command: `commands::models::get_active_gpu_vram_status` uses DXGI to report active GPU local memory usage and budget.
- New frontend component: `src/components/footer/VramMeter.tsx` shows `AivoRelay used/budget`, `system free/total VRAM`, refreshes on demand, and displays the last update time.
- Footer integration: `src/components/footer/Footer.tsx` places VRAM meter next to the model picker and triggers refresh when the model picker is clicked.

## Platform Limitations

- **Remote STT**: Windows only (uses Windows Credential Manager for API key storage)
- **AI Replace Selection**: Windows only (uses Windows-specific selection capture via `input.rs`)
- **Connector**: Cross-platform (simple HTTP client)
