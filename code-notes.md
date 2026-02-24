# Fork Code Notes

Files that differentiate this fork from the original [cjpais/Handy](https://github.com/cjpais/Handy).

## Fork-Specific Files

### Backend (Rust)

| File | Purpose |
| --- | --- |
| `src-tauri/src/managers/connector.rs` | Main connector HTTP server. |
| `src-tauri/src/commands/connector.rs` | Commands: status, queue, cancel. |
| `src-tauri/src/managers/remote_stt.rs` | Remote STT manager (OpenAI/Soniox). |
| `src-tauri/src/commands/remote_stt.rs` | Commands for remote STT & keys. |
| `src-tauri/src/managers/soniox_stt.rs` | Soniox async/non-live manager. |
| `src-tauri/src/managers/soniox_realtime.rs` | Soniox live WebSocket manager. |
| `src-tauri/src/secure_keys.rs` | Secure API key storage (Windows). |
| `src-tauri/src/plus_overlay_state.rs` | Extended overlay states. |
| `src-tauri/src/region_capture.rs` | Native region capture overlay. |
| `src-tauri/src/commands/region_capture.rs` | Commands for region capture. |
| `src-tauri/src/commands/voice_command.rs` | Voice Command Center. |
| `src-tauri/src/commands/file_transcription.rs` | File transcription logic. |
| `src-tauri/src/subtitle.rs` | Subtitle formatting (SRT/VTT). |
| `src-tauri/src/audio_toolkit/text.rs` | Text Post-Processing (stutter/filler removal). |
| `src-tauri/src/input_source.rs` | OS Language Detection. |
| `src-tauri/src/active_app.rs` | Active App Context. |
| `src-tauri/src/transcript_context.rs` | Prompt Context Cache. |
| `src-tauri/src/managers/key_listener.rs` | rdev Key Listener (Windows). |
| `src-tauri/src/commands/key_listener.rs` | Commands for key listener. |
| `src-tauri/src/language_resolver.rs` | Soniox language resolver. |
| `src-tauri/src/text_replacement_decapitalize.rs` | Decapitalize trigger. |

### Frontend (React/TypeScript)

| File | Purpose |
| --- | --- |
| `src/components/settings/remote-stt/RemoteSttSettings.tsx` | Remote STT UI. |
| `src/components/settings/SonioxContextEditor.tsx` | Soniox context editor. |
| `src/components/settings/ai-replace/AiReplaceSelectionSettings.tsx` | AI Replace config UI. |
| `src/components/settings/advanced/AiReplaceSettings.tsx` | Legacy AI Replace UI. |
| `src/components/settings/browser-connector/ConnectorStatus.tsx` | Extension status UI. |
| `src/components/icons/SendingIcon.tsx` | Upload arrow icon. |
| `src/overlay/plus_overlay_states.ts` | Types for extended overlay states. |
| `src/region-capture/RegionCaptureOverlay.tsx` | Native region selection React comp. |
| `src/region-capture/RegionCaptureOverlay.css` | Styles for RegionCapture. |
| `src/command-confirm/CommandConfirmOverlay.tsx` | Voice Command confirmation popup. |
| `src/command-confirm/CommandConfirmOverlay.css` | Styles for command popup. |
| `src/soniox-live-preview/SonioxLivePreview.tsx` | Live preview window UI. |
| `src/lib/utils/previewHotkeys.ts` | Preview hotkeys logic. |
| `src/components/ui/HotkeyCapture.tsx` | Hotkey capture UI. |
| `src/soniox-live-preview/SonioxLivePreview.css` | Styles for live preview. |
| `src/components/settings/voice-commands/VoiceCommandSettings.tsx` | Voice Command settings UI. |
| `src/components/settings/transcribe-file/TranscribeFileSettings.tsx` | Transcribe File UI. |
| `src/components/settings/text-replacement/TextReplacementSettings.tsx` | Text Replacement rules UI. |
| `src/components/settings/audio-processing/AudioProcessingSettings.tsx` | Audio processing UI. |
| `src/components/settings/debug/ShortcutEngineSelector.tsx` | Shortcut engine toggle UI. |
| `src/stores/transcribeFileStore.ts` | Session store for file transcription. |
| `src/lib/constants/sonioxLanguages.ts` | Soniox languages mapping. |

### Development & Build Tools

| File | Purpose |
| --- | --- |
| `build-local.ps1` | Local rebuild script (Windows). |
| `build-unsigned.js` | Unsigned build Node.js script. |

## Fork-Differing Existing Files

### Backend Core Logic

| File | Current State |
| --- | --- |
| `src-tauri/src/actions.rs` | Shortcut actions, variable resolution. |
| `src-tauri/src/overlay.rs` | Overlay states, preview window helpers. |
| `src-tauri/src/settings.rs` | Fork-specific settings & features. |
| `src-tauri/src/lib.rs` | Registers managers, commands, tray. |
| `src-tauri/src/shortcut.rs` | Dual-engine shortcut bindings. |
| `src-tauri/src/clipboard.rs` | Clipboard behavior. |
| `src-tauri/src/input.rs` | Selection capture utilities. |
| `src-tauri/src/tray.rs` | Custom tray menu. |

### Backend Support

| File | Current State |
| --- | --- |
| `src-tauri/src/commands/mod.rs` | Exports custom commands. |
| `src-tauri/src/managers/mod.rs` | Exports custom managers. |
| `src-tauri/src/audio_toolkit/mod.rs` | Includes `encode_wav_bytes()`. |
| `src-tauri/src/audio_toolkit/audio/utils.rs` | WAV encoding utils. |
| `src-tauri/src/audio_toolkit/audio/recorder.rs` | Mic error handling logic. |
| `src-tauri/src/managers/transcription.rs` | Local STT runtime (SenseVoice). |
| `src-tauri/src/commands/file_transcription.rs` | Soniox async integration overrides. |
| `src-tauri/src/managers/soniox_stt.rs` | Soniox language handling. |
| `src-tauri/src/managers/soniox_realtime.rs` | Soniox live language/previews. |
| `src-tauri/src/utils.rs` | Central cancellation path. |
| `src-tauri/Cargo.toml` | Extra crates (`keyring`, `reqwest`, `axum`, etc). |
| `src-tauri/resources/default_settings.json` | Default settings for fork. |

### Frontend Settings UI

| File | Current State |
| --- | --- |
| `src/components/icons/index.ts` | Exports icons. |
| `src/components/settings/advanced/AdvancedSettings.tsx` | Extra startup/paste toggles. |
| `src/components/settings/browser-connector/BrowserConnectorSettings.tsx` | Extension/screenshot UI. |
| `src/components/settings/general/GeneralSettings.tsx` | Fork settings layout. |
| `src/components/settings/user-interface/UserInterfaceSettings.tsx` | Soniox live preview UI. |
| `src/components/Sidebar.tsx` | Navigation for fork settings. |
| `src/hooks/useSettings.ts` | Fork settings hooks. |
| `src/components/settings/remote-stt/RemoteSttSettings.tsx` | Soniox hints input. |
| `src/components/settings/TranscriptionProfiles.tsx` | Provider-aware languages. |
| `src/components/settings/TranscriptionSystemPrompt.tsx` | Prompt limits handling. |
| `src/components/settings/TranslateToEnglish.tsx` | Soniox-aware UI state. |
| `src/lib/constants/languages.ts` | Custom profile languages. |
| `src/stores/settingsStore.ts` | Store for fork settings. |
| `src/i18n/locales/en/translation.json` | EN strings. |
| `src/i18n/locales/ru/translation.json` | RU strings. |
| `src/bindings.ts` | Generated Tauri bindings. |

### Other Fork-Differing Files

| File | Current State |
| --- | --- |
| `src/App.tsx` | Fork specific event listeners. |
| `src/components/model-selector/ModelSelector.tsx` | Soniox behavior support. |
| `src/components/onboarding/Onboarding.tsx` | Remote STT wizards. |
| `src/overlay/RecordingOverlay.tsx` | Extended error/sending states. |
| `src/overlay/RecordingOverlay.css` | Styles for error state. |
| `vite.config.ts` | Multi-entry target for live preview. |

## Other Context Files

| File | Purpose |
| --- | --- |
| `src-tauri/src/llm_client.rs` | LLM API client. |
| `src-tauri/src/managers/model.rs` | Local model definitions (`EngineType`). |
| `src-tauri/src/commands/models.rs` | Defines `get_active_gpu_vram_status`. |
| `src/components/footer/VramMeter.tsx` | Frontend VRAM meter UI. |
| `src/components/footer/Footer.tsx` | Footer integration for VRAM meter. |
