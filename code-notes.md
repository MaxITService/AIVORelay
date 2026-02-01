# Fork Code Notes

> **This is the CUDA branch (`cuda-integration`).**
> It differs significantly from the **main branch** (Vulkan). See [CUDA.md](CUDA.md) for build instructions and differences.

Files that differentiate this fork from the original [cjpais/Handy](https://github.com/cjpais/Handy).

## New Files (Fork-Specific)

### Backend (Rust)

| File                                           | Purpose                                                                                                                                                                                                                                                                                                                                                    |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/managers/connector.rs`          | **Main connector module**: HTTP server (port 38243) for extension communication. Extension polls `GET /messages` with Bearer auth, AivoRelay returns `{cursor, messages[], config, passwordUpdate?}`. Handles text messages, bundle (with image attachments via `/blob/*`), and keepalive messages. **Includes two-phase password rotation** for security. |
| `src-tauri/src/commands/connector.rs`          | Tauri commands for connector: `connector_get_status`, `connector_is_online`, `connector_start_server`, `connector_stop_server`, `connector_queue_message`.                                                                                                                                                                                                 |
| `src-tauri/src/managers/remote_stt.rs`         | Remote Speech-to-Text manager. Handles OpenAI-compatible API calls, WAV encoding, API key storage (Windows Credential Manager), debug logging.                                                                                                                                                                                                             |
| `src-tauri/src/commands/remote_stt.rs`         | Tauri commands exposing Remote STT functionality to frontend: `remote_stt_has_api_key`, `remote_stt_set_api_key`, `remote_stt_test_connection`, etc.                                                                                                                                                                                                       |
| `src-tauri/src/secure_keys.rs`                 | **Secure API key storage** (Windows only): Unified interface for storing all LLM API keys (Remote STT, Post-Processing, AI Replace) in Windows Credential Manager. Includes migration logic from JSON settings.                                                                                                                                            |
| `src-tauri/src/plus_overlay_state.rs`          | Extended overlay states for Remote STT error display. Categorizes errors (TLS, timeout, network, server), emits typed payloads to overlay, auto-hides after 3s.                                                                                                                                                                                            |
| `src-tauri/src/region_capture.rs`              | **Native region capture** (Windows only): Captures all monitors into single canvas, opens full-screen overlay for region selection with resize handles. Returns cropped PNG bytes directly to connector without disk I/O.                                                                                                                                  |
| `src-tauri/src/commands/region_capture.rs`     | Tauri commands for region capture overlay: `region_capture_confirm`, `region_capture_cancel`.                                                                                                                                                                                                                                                              |
| `src-tauri/src/commands/voice_command.rs`      | **Voice Command Center** (Windows only): Tauri command `execute_voice_command` runs approved PowerShell commands after user confirmation. Includes safety validation, non-blocking execution for silent commands, and support for `pwsh.exe` and `wt.exe`. |
| `src-tauri/src/commands/file_transcription.rs` | **File Transcription**: Handles logic for transcribing audio files. Decodes various audio formats (wav, mp3, etc.), manages output formats, and coordinates with local/remote transcription providers.                                                                                                                      |
| `src-tauri/src/subtitle.rs`                    | **Subtitle Formatting**: Logic for generating timestamped subtitles (SRT/VTT). Used by `file_transcription.rs` to structure transcription segments into standard subtitle formats.                                                                                                                                                                          |
| `src-tauri/src/audio_toolkit/text.rs`          | **Text Post-Processing**: Logic for cleaning up transcriptions, including collapsing repeated 1-2 letter stutters (e.g., "I-I" → "I") and filtering filler words ("uhm", "uh").                                                                                                                                                                             |
| `src-tauri/src/input_source.rs`                | **OS Language Detection**: Utilities to detect the current system input language, used for automatic language switching in transcription profiles.                                                                                                                                                                                                         |
| `src-tauri/src/managers/key_listener.rs`       | **rdev Key Listener** (Windows): Low-level keyboard hook using rdev library. Tracks modifier state, parses shortcut strings (e.g., "ctrl+shift+a", "caps lock"), emits `rdev-shortcut` events. Supports keys that Tauri can't handle: CapsLock, NumLock, ScrollLock, Pause, modifier-only shortcuts.                                                       |
| `src-tauri/src/commands/key_listener.rs`       | Tauri commands for key listener: `register_rdev_shortcut`, `unregister_rdev_shortcut`, `is_rdev_shortcut_registered`.                                                                                                                                                                                                                                       |

### Frontend (React/TypeScript)

| File                                                                   | Purpose                                                                                                                                                                 |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/settings/remote-stt/RemoteSttSettings.tsx`             | UI for Remote STT configuration: base URL, model ID, API key management, connection testing, debug log viewer.                                                          |
| `src/components/settings/advanced/AiReplaceSettings.tsx`               | UI for AI Replace feature: system/user prompts, max chars limit, "no selection" mode toggle.                                                                            |
| `src/components/settings/browser-connector/ConnectorStatus.tsx`        | Extension status indicator component showing online/offline status with "last seen" time when offline.                                                                  |
| `src/components/icons/SendingIcon.tsx`                                 | Monochrome SVG icon (upload arrow) for "sending" overlay state. Matches pink style (`#FAA2CA`) of other icons.                                                          |
| `src/overlay/plus_overlay_states.ts`                                   | TypeScript types for extended overlay states (`error`, `sending`). Error category enum and display text mapping.                                                        |
| `src/region-capture/RegionCaptureOverlay.tsx`                          | React component for native region selection: state machine (idle→creating→selected), mouse handling, resize handles.                                                    |
| `src/region-capture/RegionCaptureOverlay.css`                          | Styles for region capture overlay: dim areas, selection border, resize handles, cursor states.                                                                          |
| `src/command-confirm/CommandConfirmOverlay.tsx`                        | **Voice Command Center**: Confirmation popup showing suggested PowerShell command with Run/Edit/Cancel buttons.                                                         |
| `src/command-confirm/CommandConfirmOverlay.css`                        | Styles for command confirmation overlay: glassmorphism, dark theme, vibrant accent colors.                                                                              |
| `src/components/settings/voice-commands/VoiceCommandSettings.tsx`      | Settings UI for managing predefined voice commands, similarity thresholds, and LLM fallback toggle.                                                                     |
| `src/components/settings/transcribe-file/TranscribeFileSettings.tsx`   | UI for "Transcribe Audio File" feature: Drag-and-drop zone, file info, output format selection (Text/SRT/VTT), optional model override, and results display.            |
| `src/components/settings/text-replacement/TextReplacementSettings.tsx` | UI for "Text Replacement" feature: Add/remove replacement rules with enable/disable toggles. Supports escape sequences for special characters (\\n, \\r\\n, \\t, \\\\), regex matching, and adjustable execution order (Before/After LLM). |
| `src/components/settings/audio-processing/AudioProcessingSettings.tsx` | UI for audio processing settings: VAD sensitivity, stutter collapsing, and transcription cleaning options.                                                                                                                                                                                           |
| `src/components/settings/debug/ShortcutEngineSelector.tsx`             | **Shortcut Engine Selector** (Windows): UI for switching between Tauri (high-perf, limited keys) and rdev (all keys, higher CPU) engines. Shows incompatible shortcuts warning, requires app restart. Located in Debug → Experimental Features.                                                     |
| `src/stores/transcribeFileStore.ts`                                    | Session store for Transcribe File UI state (selected file, output mode, profile selection, results).                                                                    |

## Modified Files

### Backend Core Logic

| File                         | Changes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/actions.rs`   | Added new shortcut actions: `AiReplaceSelectionAction`, `SendToExtensionAction`, `SendToExtensionWithSelectionAction`, `SendScreenshotToExtensionAction`. These handle the new voice-to-LLM, connector, and screenshot workflows. **Uses `show_sending_overlay()` for Remote STT instead of `show_transcribing_overlay()`.**                                                                                                                                                                                          |
| `src-tauri/src/overlay.rs`   | Added `show_sending_overlay()` function, made `force_overlay_topmost()` public for reuse.                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `src-tauri/src/settings.rs`  | Extended `AppSettings` with: `transcription_provider`, `remote_stt` settings, `ai_replace_*` fields, `connector_*` fields (including `connector_password` for auth), `screenshot_*` fields, individual push-to-talk settings, `shortcut_engine` (Windows). Added `RemoteSttSettings`, `TranscriptionProvider`, `ShortcutEngine` enums. Added explicit `store.save()` in `write_settings()` to prevent race conditions on restart. |
| `src-tauri/src/lib.rs`       | Registered new managers (`RemoteSttManager`, `ConnectorManager`) and commands including individual push-to-talk commands and screenshot settings commands. Starts connector server on app init. Handles tray icon creation and event loop.                                                                    |
| `src-tauri/src/shortcut.rs`  | Added shortcut bindings for new actions (AI Replace, Send to Extension, Send Screenshot to Extension). Added commands for individual push-to-talk settings and screenshot settings, plus logic to use per-binding push-to-talk instead of global setting for fork-specific actions. Integrated OS language detection for automatic profile switching. **Added dual-engine support (Windows)**: conditionally starts rdev listener, routes shortcuts to Tauri or rdev based on compatibility, clears incompatible bindings on engine switch. |
| `src-tauri/src/clipboard.rs` | Enhanced clipboard handling for AI Replace selection capture.                                                                                                                                                                                                                                                                                                                        |
| `src-tauri/src/input.rs`     | Added selection capture utilities for Windows.                                                                                                                                                                                                                                                                                                                                       |
| `src-tauri/src/tray.rs`      | Custom tray menu implementation: added "Copy Last Transcript" action and access to quick settings.                                                                                                                                                                                                                                                                                |

### Backend Support

| File                                         | Changes                                                                                                                                                           |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/commands/mod.rs`              | Exported new `remote_stt` and `connector` commands modules.                                                                                                       |
| `src-tauri/src/managers/mod.rs`              | Exported `remote_stt` and `connector` manager modules.                                                                                                            |
| `src-tauri/src/audio_toolkit/mod.rs`         | Added `encode_wav_bytes()` for Remote STT API.                                                                                                                    |
| `src-tauri/src/audio_toolkit/audio/utils.rs` | WAV encoding utilities.                                                                                                                                           |
| `src-tauri/src/audio_toolkit/audio/recorder.rs` | Mic error handling logic: detects and reports when the microphone is unavailable or used by another process. |
| `src-tauri/Cargo.toml`                       | Added dependencies: `keyring` (credential storage), `reqwest` features, `tiny_http` (HTTP server for connector), `notify` (file system watching for screenshots), `windows` crates for input language detection. |
| `src-tauri/resources/default_settings.json`  | Default values for new settings.                                                                                                                                  |

### Frontend Settings UI

| File                                                                     | Changes                                                                                              |
| ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `src/components/icons/index.ts`                                          | Exports `SendingIcon` component.                                                                     |
| `src/components/settings/advanced/AdvancedSettings.tsx`                  | Added Remote STT and AI Replace settings sections.                                                   |
| `src/components/settings/browser-connector/BrowserConnectorSettings.tsx` | Added extension status indicator section and screenshot settings (capture command, folder, timeout). |
| `src/components/settings/general/GeneralSettings.tsx`                    | Minor adjustments for new settings layout.                                                           |
| `src/components/Sidebar.tsx`                                             | Navigation for new settings sections.                                                                |
| `src/hooks/useSettings.ts`                                               | Hooks for new settings: `setTranscriptionProvider`, `updateRemoteStt*`, `updateAiReplace*`.          |
| `src/stores/settingsStore.ts`                                            | State management for new settings.                                                                   |
| `src/i18n/locales/en/translation.json`                                   | Translations for all new UI strings.                                                                 |
| `src/bindings.ts`                                                        | Auto-generated Tauri command bindings (includes remote_stt commands).                                |

### Other Modified

| File                                       | Changes                                                                                                                                                                                |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/App.tsx`                              | Event listeners for new features (remote-stt-error, ai-replace-error, screenshot-error).                                                                                               |
| `src/components/model-selector/*`          | Adjusted for transcription provider switching.                                                                                                                                         |
| `src/components/onboarding/Onboarding.tsx` | Updated for Remote STT option.                                                                                                                                                         |
| `src/overlay/RecordingOverlay.tsx`         | Extended to handle `error` and `sending` states with categorized error messages. Uses `SendingIcon` for "sending" state. Accepts extended payload object instead of string-only state. |
| `src/overlay/RecordingOverlay.css`         | Added `.error-text` and `.overlay-error` styles for error state display.                                                                                                               |

## Feature → File Mapping

### Remote STT API

```
User configures in UI
    └─► RemoteSttSettings.tsx
            └─► useSettings.ts → settings.rs
                    └─► remote_stt.rs (manager)
                            └─► OpenAI-compatible API
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
| Add new AI Replace setting          | `settings.rs` → add field, `AiReplaceSettings.tsx` → add UI                              |
| Change Remote STT API handling      | `managers/remote_stt.rs` → `transcribe()`                                                |
| Add new shortcut action             | `actions.rs` → impl `ShortcutAction`, register in `ACTION_MAP`                           |
| Change selection capture logic      | `input.rs` (Windows-specific)                                                            |
| Add new Tauri command               | `commands/*.rs` → add fn, `commands/mod.rs` → export                                     |
| Change extension status timeout     | `managers/connector.rs` → `EXTENSION_TIMEOUT_SECS` constant                              |
| Customize status display            | `ConnectorStatus.tsx`                                                                    |
| Change connector password           | `settings.rs` → `connector_password` field, `BrowserConnectorSettings.tsx` → password UI |
| Add/modify transcription profiles   | `settings.rs` → `TranscriptionProfile`, `shortcut.rs` → profile commands                 |
| Change profile system prompt limits | `TranscriptionSystemPrompt.tsx` → `getModelPromptInfo()`, `managers/remote_stt.rs`       |

## Key Data Structures

| Structure               | File                    | Purpose                                                                          |
| ----------------------- | ----------------------- | -------------------------------------------------------------------------------- |
| `AppSettings`           | `settings.rs`           | All app settings, includes `ai_replace_*`, `remote_stt`, `connector_*`           |
| `RemoteSttSettings`     | `settings.rs`           | base_url, model_id, debug_mode, debug_capture                                    |
| `TranscriptionProfile`  | `settings.rs`           | Custom shortcut profile: id, name, language, translate_to_english, system_prompt |
| `TranscriptionProvider` | `settings.rs`           | Enum: `Local`, `RemoteOpenAiCompatible`                                          |
| `ShortcutAction` trait  | `actions.rs`            | Interface for all shortcut actions (start/stop)                                  |
| `ACTION_MAP`            | `actions.rs`            | Registry of all available shortcut actions                                       |
| `ConnectorManager`      | `managers/connector.rs` | HTTP server tracking extension status via polling                                |
| `ConnectorStatus`       | `managers/connector.rs` | Status struct with `online`, `last_poll`, `server_running` fields                |

## Change Impact

| If you change...         | Check also...                                                              |
| ------------------------ | -------------------------------------------------------------------------- |
| `AppSettings` fields     | `default_settings.json`, `useSettings.ts`, `settingsStore.ts`              |
| Tauri commands           | Run `bun run tauri dev` to regenerate `bindings.ts`                        |
| Remote STT API format    | `encode_wav_bytes()` in audio_toolkit                                      |
| Connector message format | Extension expects `{id, type, text, ts, attachments?}` from polling server |
| Connector auth           | Extension uses `Authorization: Bearer <password>` header                   |
| Prompt templates         | Variables: `${instruction}` (voice), `${output}` (selected/input text)     |
| Quick Tap (AI Replace)   | Skips STT if < 800ms; uses `ai_replace_quick_tap_system_prompt`            |
| Allow No Voice           | If enabled, sends `${output}` only with specific "No Voice" system prompt  |

## Platform Limitations

- **Remote STT**: Windows only (uses Windows Credential Manager for API key storage)
- **AI Replace Selection**: Windows only (uses Windows-specific selection capture via `input.rs`)
- **Connector**: Cross-platform (simple HTTP client)
