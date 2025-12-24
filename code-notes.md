# Fork Code Notes

Files that differentiate this fork from the original [cjpais/Handy](https://github.com/cjpais/Handy).

## New Files (Fork-Specific)

### Backend (Rust)

| File | Purpose |
|------|---------|
| `src-tauri/src/connector.rs` | HTTP client for sending transcriptions to external services (Handy Connector). Sends JSON payload `{text, ts}` to configurable endpoint. |
| `src-tauri/src/managers/connector.rs` | Connector Manager with HTTP server for extension communication. Tracks extension online/offline status via polling, handles keepalive messages. |
| `src-tauri/src/commands/connector.rs` | Tauri commands for connector: `connector_get_status`, `connector_is_online`, `connector_start_server`, `connector_stop_server`, `connector_queue_message`. |
| `src-tauri/src/managers/remote_stt.rs` | Remote Speech-to-Text manager. Handles OpenAI-compatible API calls, WAV encoding, API key storage (Windows Credential Manager), debug logging. |
| `src-tauri/src/commands/remote_stt.rs` | Tauri commands exposing Remote STT functionality to frontend: `remote_stt_has_api_key`, `remote_stt_set_api_key`, `remote_stt_test_connection`, etc. |

### Frontend (React/TypeScript)

| File | Purpose |
|------|---------|
| `src/components/settings/remote-stt/RemoteSttSettings.tsx` | UI for Remote STT configuration: base URL, model ID, API key management, connection testing, debug log viewer. |
| `src/components/settings/advanced/AiReplaceSettings.tsx` | UI for AI Replace feature: system/user prompts, max chars limit, "no selection" mode toggle. |
| `src/components/settings/browser-connector/ConnectorStatus.tsx` | Extension status indicator component showing online/offline status with "last seen" time when offline. |

## Modified Files

### Backend Core Logic

| File | Changes |
|------|---------|
| `src-tauri/src/actions.rs` | Added new shortcut actions: `AiReplaceSelectionAction`, `SendToExtensionAction`, `SendToExtensionWithSelectionAction`. These handle the new voice-to-LLM and connector workflows. |
| `src-tauri/src/settings.rs` | Extended `AppSettings` with: `transcription_provider`, `remote_stt` settings, `ai_replace_*` fields, `connector_*` fields, individual push-to-talk settings (`send_to_extension_push_to_talk`, `send_to_extension_with_selection_push_to_talk`, `ai_replace_selection_push_to_talk`). Added `RemoteSttSettings`, `TranscriptionProvider` enum, `default_true()` helper. |
| `src-tauri/src/lib.rs` | Registered new managers (`RemoteSttManager`, `ConnectorManager`) and commands including individual push-to-talk commands. Starts connector server on app init. |
| `src-tauri/src/shortcut.rs` | Added shortcut bindings for new actions (AI Replace, Send to Extension). Added commands for individual push-to-talk settings and logic to use per-binding push-to-talk instead of global setting for fork-specific actions. |
| `src-tauri/src/clipboard.rs` | Enhanced clipboard handling for AI Replace selection capture. |
| `src-tauri/src/input.rs` | Added selection capture utilities for Windows. |

### Backend Support

| File | Changes |
|------|---------|
| `src-tauri/src/commands/mod.rs` | Exported new `remote_stt` and `connector` commands modules. |
| `src-tauri/src/managers/mod.rs` | Exported `remote_stt` and `connector` manager modules. |
| `src-tauri/src/audio_toolkit/mod.rs` | Added `encode_wav_bytes()` for Remote STT API. |
| `src-tauri/src/audio_toolkit/audio/utils.rs` | WAV encoding utilities. |
| `src-tauri/Cargo.toml` | Added dependencies: `keyring` (credential storage), `reqwest` features, `tiny_http` (HTTP server for connector). |
| `src-tauri/resources/default_settings.json` | Default values for new settings. |

### Frontend Settings UI

| File | Changes |
|------|---------|
| `src/components/settings/advanced/AdvancedSettings.tsx` | Added Remote STT and AI Replace settings sections. |
| `src/components/settings/browser-connector/BrowserConnectorSettings.tsx` | Added extension status indicator section. |
| `src/components/settings/general/GeneralSettings.tsx` | Minor adjustments for new settings layout. |
| `src/components/Sidebar.tsx` | Navigation for new settings sections. |
| `src/hooks/useSettings.ts` | Hooks for new settings: `setTranscriptionProvider`, `updateRemoteStt*`, `updateAiReplace*`. |
| `src/stores/settingsStore.ts` | State management for new settings. |
| `src/i18n/locales/en/translation.json` | Translations for all new UI strings. |
| `src/bindings.ts` | Auto-generated Tauri command bindings (includes remote_stt commands). |

### Other Modified

| File | Changes |
|------|---------|
| `src/App.tsx` | Event listeners for new features (remote-stt-error, ai-replace-error). |
| `src/components/model-selector/*` | Adjusted for transcription provider switching. |
| `src/components/onboarding/Onboarding.tsx` | Updated for Remote STT option. |

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
            └─► connector.rs → HTTP POST to Handy Connector
```

### Extension Status Tracking
```
Extension polls Handy server
    └─► managers/connector.rs (HTTP server on port 63155)
            └─► tracks lastPoll timestamp
            └─► emits "extension-status-changed" event
                    └─► ConnectorStatus.tsx updates UI
                            └─► Shows 🟢 Online / 🔴 Offline + "Last seen"
```

## Entry Points for Common Tasks

| Task | Start Here |
|------|------------|
| Change AI Replace behavior | `actions.rs` → `AiReplaceSelectionAction::stop()` |
| Add new AI Replace setting | `settings.rs` → add field, `AiReplaceSettings.tsx` → add UI |
| Change message format for Connector | `actions.rs` → `build_extension_message()` |
| Change Remote STT API handling | `managers/remote_stt.rs` → `transcribe()` |
| Add new shortcut action | `actions.rs` → impl `ShortcutAction`, register in `ACTION_MAP` |
| Change selection capture logic | `input.rs` (Windows-specific) |
| Add new Tauri command | `commands/*.rs` → add fn, `commands/mod.rs` → export |
| Change extension status timeout | `managers/connector.rs` → `EXTENSION_TIMEOUT_SECS` constant |
| Customize status display | `ConnectorStatus.tsx` |

## Key Data Structures

| Structure | File | Purpose |
|-----------|------|---------|
| `AppSettings` | `settings.rs` | All app settings, includes `ai_replace_*`, `remote_stt`, `connector_*` |
| `RemoteSttSettings` | `settings.rs` | base_url, model_id, debug_mode, debug_capture |
| `TranscriptionProvider` | `settings.rs` | Enum: `Local`, `RemoteOpenAiCompatible` |
| `ShortcutAction` trait | `actions.rs` | Interface for all shortcut actions (start/stop) |
| `ACTION_MAP` | `actions.rs` | Registry of all available shortcut actions |
| `ConnectorManager` | `managers/connector.rs` | HTTP server tracking extension status via polling |
| `ConnectorStatus` | `managers/connector.rs` | Status struct with `online`, `last_poll`, `server_running` fields |

## Change Impact

| If you change... | Check also... |
|------------------|---------------|
| `AppSettings` fields | `default_settings.json`, `useSettings.ts`, `settingsStore.ts` |
| Tauri commands | Run `bun run tauri dev` to regenerate `bindings.ts` |
| Remote STT API format | `encode_wav_bytes()` in audio_toolkit |
| Connector message format | Handy Connector extension expects `{text, ts}` |
| Prompt templates | Variables: `${instruction}` (voice), `${output}` (selected text) |

## Platform Limitations

- **Remote STT**: Windows only (uses Windows Credential Manager for API key storage)
- **AI Replace Selection**: Windows only (uses Windows-specific selection capture via `input.rs`)
- **Connector**: Cross-platform (simple HTTP client)
