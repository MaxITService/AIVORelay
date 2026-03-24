# Fork Code Notes

Files that differentiate this fork from the original [cjpais/Handy](https://github.com/cjpais/Handy).

Branch note:
- For the current `cuda-integration -> main` file diff and CUDA dependency wiring, use [[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]].
- This file remains fork-vs-upstream focused.

## Fork-Added Files

Files that are added by this fork rather than upstream files that were modified.

### Backend (Rust)

| File | Purpose |
| --- | --- |
| `src-tauri/src/managers/connector.rs` | Main connector HTTP server. |
| `src-tauri/src/commands/connector.rs` | Commands: status, queue, cancel, bundled extension export. |
| `src-tauri/src/managers/remote_stt.rs` | Remote STT manager (OpenAI/Soniox). |
| `src-tauri/src/commands/remote_stt.rs` | Commands for remote STT & keys. |
| `src-tauri/src/managers/deepgram_stt.rs` | Deepgram non-live/live-finalize transcription manager. |
| `src-tauri/src/secure_keys.rs` | Secure API key storage (Windows). |
| `src-tauri/src/plus_overlay_state.rs` | Extended overlay states. |
| `src-tauri/src/region_capture.rs` | Native region capture overlay. |
| `src-tauri/src/commands/region_capture.rs` | Commands for region capture. |
| `src-tauri/src/commands/voice_command.rs` | Voice Command Center. |
| `src-tauri/src/commands/live_sound_transcription.rs` | Live Sound Transcription page command surface. |
| `src-tauri/src/file_transcription_diarization.rs` | Shared diarized file-transcription temp session + speaker re-apply helpers. |
| `src-tauri/src/subtitle.rs` | Subtitle formatting (SRT/VTT). |
| `src-tauri/src/audio_toolkit/text.rs` | Text Post-Processing (stutter/filler removal). |
| `src-tauri/src/input_source.rs` | OS Language Detection. |
| `src-tauri/src/active_app.rs` | Active App Context. |
| `src-tauri/src/transcript_context.rs` | Prompt Context Cache. |
| `src-tauri/src/url_security.rs` | Canonical provider URLs and HTTPS/HTTP override validation for Remote STT and LLM endpoints. |
| `src-tauri/src/managers/key_listener.rs` | rdev Key Listener (Windows). |
| `src-tauri/src/commands/key_listener.rs` | Commands for key listener. |
| `src-tauri/src/shortcut_handy_keys.rs` | Ported upstream HandyKeys shortcut backend and backend-side shortcut recording. |
| `src-tauri/src/language_resolver.rs` | Soniox language resolver. |
| `src-tauri/src/text_replacement_decapitalize.rs` | Decapitalize trigger. |

### Frontend (React/TypeScript)

| File | Purpose |
| --- | --- |
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
| `src/soniox-live-preview/SonioxLivePreview.tsx` | Live preview window UI, drag grip, edge resize handles, persisted geometry, preview delete actions. |
| `src/lib/utils/previewHotkeys.ts` | Preview hotkeys logic. |
| `src/components/ui/HotkeyCapture.tsx` | Hotkey capture UI. |
| `src/soniox-live-preview/SonioxLivePreview.css` | Styles for live preview, drag grip, and resize handles. |
| `src/components/settings/voice-commands/VoiceCommandSettings.tsx` | Voice Command settings UI. |
| `src/components/settings/live-sound-transcription/LiveSoundTranscriptionSettings.tsx` | Live Sound Transcription page with in-page diarized transcript and source/device controls. |
| `src/components/settings/text-replacement/TextReplacementSettings.tsx` | Text Replacement rules UI. |
| `src/components/settings/audio-processing/AudioProcessingSettings.tsx` | Audio processing UI. |
| `src/components/settings/GlobalShortcutInput.tsx` | Browser-side shortcut capture for Tauri/rdev engines. |
| `src/components/settings/HandyKeysShortcutInput.tsx` | Backend-side shortcut capture UI for HandyKeys engine. |
| `src/components/settings/debug/ShortcutEngineSelector.tsx` | Shortcut engine toggle UI. |
| `src/lib/constants/sonioxLanguages.ts` | Soniox languages mapping. |
| `src/lib/constants/remoteSttProviders.ts` | Remote STT preset metadata for Groq/OpenAI/custom URL handling. |

### Development & Build Tools

| File | Purpose |
| --- | --- |
| `build-local.ps1` | Local rebuild script (Windows). |
| `build-unsigned.js` | Unsigned build Node.js script. |
| `.github/workflows/code-quality.yml` | Combined PR lint/format workflow with path filters and cancellation of stale runs. |
| `.github/release-notes/*.md` | Checked-in branch-specific GitHub release body Markdown picked up by release workflows. |
| `.AGENTS/rebuild-browser-connector-bundle.ps1` | Rebuilds the tracked bundled browser-extension zip from the sibling `AIVORelay-relay` repo. |

## Fork-Differing Existing Files

### Backend Core Logic

| File | Current State |
| --- | --- |
| `src-tauri/src/actions.rs` | Shortcut actions, variable resolution, preview delete actions. |
| `src-tauri/src/overlay.rs` | Overlay states, preview window helpers, live preview geometry constraints, preview action appearance payload. |
| `src-tauri/src/settings.rs` | Fork-specific settings & features, including live preview action toggles/hotkeys and preview action bindings. |
| `src-tauri/src/lib.rs` | Registers managers, commands, tray. |
| `src-tauri/src/shortcut.rs` | Multi-engine shortcut bindings (Tauri/rdev/HandyKeys), live preview geometry persistence commands, preview action settings commands, preview delete-last-word global hotkey sync. |
| `src-tauri/src/clipboard.rs` | Clipboard behavior. |
| `src-tauri/src/input.rs` | Selection capture utilities. |
| `src-tauri/src/tray.rs` | Custom tray menu. |

### Backend Support

| File | Current State |
| --- | --- |
| `src-tauri/src/commands/mod.rs` | Exports custom commands. |
| `src-tauri/src/managers/mod.rs` | Exports custom managers, including live sound transcript state. |
| `src-tauri/src/managers/live_sound_transcription.rs` | Live Sound page runtime state, transcript events, and speaker-segment payloads. |
| `src-tauri/src/audio_toolkit/mod.rs` | Includes `encode_wav_bytes()`. |
| `src-tauri/src/audio_toolkit/audio/utils.rs` | WAV encoding utils. |
| `src-tauri/src/audio_toolkit/audio/recorder.rs` | Audio capture stream logic, including Windows output loopback support. |
| `src-tauri/src/managers/audio.rs` | Routes recordings between mic capture and Windows output loopback for live sound. |
| `src-tauri/src/managers/transcription.rs` | Local STT runtime (SenseVoice). |
| `src-tauri/src/commands/file_transcription.rs` | Soniox async integration overrides and diarized speaker-session handling. |
| `src-tauri/src/settings.rs` | Also stores saved diarization speaker-name set profiles for file transcription. |
| `src-tauri/src/shortcut.rs` | Includes persisted setting update commands for diarization speaker-name sets. |
| `src-tauri/src/managers/soniox_stt.rs` | Soniox language handling. |
| `src-tauri/src/managers/soniox_realtime.rs` | Soniox live language/previews plus speaker-aware live page updates. |
| `src-tauri/src/managers/deepgram_realtime.rs` | Deepgram live preview/finalize flow plus speaker-aware live page updates. |
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
| `src/components/settings/remote-stt/RemoteSttSettings.tsx` | Soniox + Deepgram provider settings. |
| `src/components/settings/TranscriptionProfiles.tsx` | Provider-aware languages. |
| `src/components/settings/transcribe-file/TranscribeFileSettings.tsx` | File transcription UI, including diarization speaker-name set profiles. |
| `src/components/settings/TranscriptionSystemPrompt.tsx` | Prompt limits handling. |
| `src/components/settings/TranslateToEnglish.tsx` | Soniox/Deepgram-aware UI state. |
| `src/lib/constants/languages.ts` | Custom profile languages. |
| `src/stores/settingsStore.ts` | Store for fork settings. |
| `src/stores/transcribeFileStore.ts` | Holds editable diarization speaker cards and bulk profile-apply helpers. |
| `src/i18n/locales/en/translation.json` | EN strings. |
| `src/i18n/locales/ru/translation.json` | RU strings. |
| `src/bindings.ts` | Generated Tauri bindings, including live preview geometry helpers. |

### Other Fork-Differing Files

| File | Current State |
| --- | --- |
| `src/App.tsx` | Fork specific event listeners. |
| `src/components/model-selector/ModelSelector.tsx` | Soniox/Deepgram behavior support. |
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

## Browser Connector Bundle

- The desktop app now ships a tracked browser-extension archive at `src-tauri/resources/browser-connector/aivorelay-extension.zip`.
- This zip is a normal bundled resource so GitHub Actions can include it without needing the sibling `AIVORelay-relay` repo at build time.
- The connector settings page exports this zip into an unpacked folder for `chrome://extensions -> Load unpacked`, then patches that exported copy with a per-export `manifest.key`, derived Chrome extension ID, exact `chrome-extension://<id>` connector origin, and a new generated connector password.
- Connector password transitions now keep accepting both the current and pending password until the extension sends `password_ack`; pending passwords are no longer auto-expired on a short TTL.
- To refresh the bundled zip after changing the extension repo, run `.AGENTS/rebuild-browser-connector-bundle.ps1` locally.
- The script copies only the runtime extension files from the sibling `AIVORelay-relay` repo and excludes `.git`, docs, demos, and other non-runtime files.
