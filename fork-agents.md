# Fork Agents Guide
ALL DEBUG IS DONE BY USER!! NOT AUTOMATED TESTS!!
NEVER Build ! User will build!
This file provides guidance for AI code agents working with this fork.

## ⚠️ Important: This is a Fork

This repository is a **fork** of [cjpais/Handy](https://github.com/cjpais/Handy).

- **Upstream**: https://github.com/cjpais/Handy (original project)
- **This fork**: Adds Windows-specific features and external service integrations

## Fork Documentation

| File | Description |
|------|-------------|
| [`code-notes.md`](code-notes.md) | **Complete list of fork-specific files and changes** — read this to understand what differs from upstream |
| [`AGENTS.md`](AGENTS.md) | Original development commands and architecture (applies to both upstream and fork) |
| [`README.md`](README.md) | Fork features overview |

When adding new features, please prefer adding them in new files instead of editing originals unless these are already fork-specific files. 

## Fork-Specific Features

### 1. Remote STT API (Windows only)
- Files: `src-tauri/src/managers/remote_stt.rs`, `src/components/settings/remote-stt/`
- Uses OpenAI-compatible `/audio/transcriptions` endpoint
- API keys stored in Windows Credential Manager via `keyring` crate

#### Transcription Flow & Status Info

When using Remote STT API, the **Recording Overlay** (`recording_overlay` window) shows visual status:

| State | Overlay Display | Description |
|-------|-----------------|-------------|
| `recording` | Red pulsing circle | User is recording audio |
| `transcribing` | Blue pulsing circle | Audio sent to API, waiting for response |
| `error` | Red text with message | API error occurred (auto-hides after 3s) |

**Error Categories** (shown in overlay):
- **Certificate Error** — TLS certificate validation failed
- **TLS Error** — TLS handshake or protocol failure
- **Connection Timeout** — Server did not respond in time
- **Network Error** — Cannot reach server (DNS, connection refused)
- **Server Error** — HTTP 4xx/5xx response from API
- **Parse Error** — Invalid JSON response
- **Transcription Error** — Generic/unknown error

**Flow after recording stops:**
1. Overlay switches from `recording` → `transcribing`
2. Audio is sent to configured API endpoint
3. On **success**: Text pasted to active app, overlay hides
4. On **error**: Overlay shows categorized error for 3 seconds, then hides; toast notification also shown via `remote-stt-error` event

**Implementation files:**
- `src-tauri/src/plus_overlay_state.rs` — Error categorization and overlay control
- `src/overlay/plus_overlay_states.ts` — TypeScript types and display text
- `src/overlay/RecordingOverlay.tsx` — Overlay UI (modified to handle extended states)

### 2. AI Replace Selection (Windows only)
- Files: `src-tauri/src/actions.rs` (AiReplaceSelectionAction), `src/components/settings/advanced/AiReplaceSettings.tsx`
- Captures selected text, sends to LLM with voice instruction, replaces selection
- Selection capture uses Windows-specific APIs in `src-tauri/src/input.rs`

### 3. Connector (Send to Extension)
- Files: `src-tauri/src/connector.rs`, `src-tauri/src/actions.rs` (SendToExtensionAction)
- HTTP POST to local server (default `http://127.0.0.1:63155/messages`)
- Designed to work with [Handy Connector](https://github.com/user/handy-connector) Chrome extension

### 4. Send Screenshot to Extension (Windows only)
- Files: `src-tauri/src/actions.rs` (SendScreenshotToExtensionAction), `src-tauri/src/managers/connector.rs` (blob serving)
- Launches external screenshot tool (default: ShareX with `-RectangleRegion`)
- Watches screenshot folder for new images using `notify` crate
- Sends bundle message with image attachment and voice instruction to extension
- Configurable: capture command, folder path, timeout, "require recent" filter, "allow without voice"
- Settings: `screenshot_capture_command`, `screenshot_folder`, `screenshot_require_recent`, `screenshot_timeout_seconds`, `screenshot_allow_no_voice`, `screenshot_no_voice_default_prompt`

## Guidelines for Agents

### When Modifying Fork Features
1. Check [`code-notes.md`](code-notes.md) to understand which files are fork-specific
2. Fork features are mostly Windows-only — use `#[cfg(target_os = "windows")]` guards
3. Settings are in `src-tauri/src/settings.rs` (look for `remote_stt`, `ai_replace_*`, `connector_*` fields)

### When Syncing with Upstream
1. Fork files in `code-notes.md` → **keep our version**
2. Files not in `code-notes.md` → **can be updated from upstream**
3. Watch for conflicts in: `settings.rs`, `actions.rs`, `lib.rs`, `App.tsx`, `useSettings.ts`

### Testing Fork Features
```bash
# Remote STT requires Windows + API key configured
# AI Replace requires Windows + LLM provider configured
# Connector requires local HTTP server running (use test-server from Handy Connector)
```

### Adding New Fork Features
1. Add new files when possible (cleaner separation from upstream)
2. Document in `code-notes.md`
3. Add translations in `src/i18n/locales/en/translation.json`
4. Consider platform guards if Windows-specific

## Upstream Tracking

Last sync point: Check git history for merge commits from upstream.

To check upstream changes:
```bash
git remote add upstream https://github.com/cjpais/Handy.git
git fetch upstream
git log HEAD..upstream/main --oneline
```

---

## 🔀 Merge Guide (Upstream Sync)

When merging upstream changes, these files will likely have conflicts. Here's how to resolve them:

### High-Conflict Files (Modified by Fork)

#### `src-tauri/src/settings.rs`
**Our additions (MUST KEEP):**
- `TranscriptionProvider` enum (`Local`, `RemoteOpenAiCompatible`)
- `RemoteSttSettings` struct (base_url, model_id, debug_mode, debug_capture)
- Fields in `AppSettings`: `transcription_provider`, `remote_stt`, `ai_replace_*`, `connector_*`, `screenshot_*`

**Merge strategy:** Keep all our additions. Accept upstream changes to other fields. If upstream adds new settings, add them alongside ours.

#### `src-tauri/src/actions.rs`
**Our additions (MUST KEEP):**
- `AiReplaceSelectionAction` struct and impl (~260 lines)
- `SendToExtensionAction` struct and impl (~240 lines)
- `SendToExtensionWithSelectionAction` struct and impl (~200 lines)
- `SendScreenshotToExtensionAction` struct and impl (~300 lines)
- `build_extension_message()` function
- `ai_replace_with_llm()` async function
- `emit_ai_replace_error()` helper
- `emit_screenshot_error()` helper
- `find_recent_image()`, `watch_for_new_image()` functions
- Entries in `ACTION_MAP` for: `ai_replace_selection`, `send_to_extension`, `send_to_extension_with_selection`, `send_screenshot_to_extension`

**Merge strategy:** Keep all our actions intact. If upstream changes `TranscribeAction`, review changes but preserve our modifications to it (remote STT support). Accept upstream additions to `ACTION_MAP`.

#### `src-tauri/src/lib.rs`
**Our additions (MUST KEEP):**
- `use managers::remote_stt::RemoteSttManager;`
- `RemoteSttManager::new()` initialization
- `.manage(Arc::new(remote_stt_manager))` state registration
- Remote STT commands in `.invoke_handler()`: `remote_stt_*`

**Merge strategy:** Keep our manager and commands. Add any new upstream managers/commands alongside ours.

#### `src-tauri/src/shortcut.rs`
**Our additions (MUST KEEP):**
- Shortcut bindings for `ai_replace_selection`, `send_to_extension`, `send_to_extension_with_selection`, `send_screenshot_to_extension`
- Commands for screenshot settings: `change_screenshot_*_setting`

**Merge strategy:** Keep our bindings. Accept upstream changes to other shortcuts.

### Medium-Conflict Files

#### `src-tauri/Cargo.toml`
**Our additions:** `keyring`, `notify` dependencies
**Merge strategy:** Keep our dependencies, accept upstream dependency updates.

#### `src/hooks/useSettings.ts`
**Our additions:** Hooks for `setTranscriptionProvider`, `updateRemoteStt*`, `updateAiReplace*`
**Merge strategy:** Keep our hooks, accept upstream hook changes.

#### `src/App.tsx`
**Our additions:** Event listeners for `remote-stt-error`, `ai-replace-error`, `screenshot-error`
**Merge strategy:** Keep our listeners, accept upstream UI changes.

#### `src-tauri/resources/default_settings.json`
**Our additions:** Default values for `ai_replace_*` settings, `screenshot_*` settings, `bindings.ai_replace_selection`, `bindings.send_screenshot_to_extension`
**Merge strategy:** Keep our defaults, add new upstream defaults.

### Fork-Only Files (No Conflict Expected)

These files are 100% ours — upstream won't have them:
- `src-tauri/src/connector.rs`
- `src-tauri/src/managers/remote_stt.rs`
- `src-tauri/src/commands/remote_stt.rs`
- `src-tauri/src/plus_overlay_state.rs` — Extended overlay states for error display
- `src/components/settings/remote-stt/RemoteSttSettings.tsx`
- `src/components/settings/advanced/AiReplaceSettings.tsx`
- `src/overlay/plus_overlay_states.ts` — TypeScript types for extended overlay

### After Merge Checklist

1. [ ] Run `bun run tauri dev` — check bindings regenerate
2. [ ] Test Remote STT connection
3. [ ] Test AI Replace with selection
4. [ ] Test Send to Extension (both modes)
5. [ ] Test Send Screenshot to Extension
6. [ ] Verify settings UI loads without errors
7. [ ] Check all fork settings persist after restart

---

## Common Patterns in This Fork

### Adding a new setting
1. Add field to `AppSettings` in `settings.rs`
2. Add default in `src-tauri/resources/default_settings.json`
3. Add hook in `src/hooks/useSettings.ts`
4. Add UI component in appropriate settings section
5. Add translations

### Adding a new shortcut action
1. Create struct implementing `ShortcutAction` in `actions.rs`
2. Register in `ACTION_MAP` at bottom of `actions.rs`
3. Add to shortcut configuration in settings

### Platform-specific code
```rust
#[cfg(target_os = "windows")]
pub fn windows_only_function() { ... }

#[cfg(not(target_os = "windows"))]
pub fn windows_only_function() {
    // Stub or error for non-Windows
}
```

## Debugging Fork Features

ALL DEBUG IS DONE BY USER!! NOT AUTOMATED TESTS!!
NEVER Build ! User will build!
### Remote STT
1. Enable debug logging: Settings → Advanced → Remote STT → Debug Capture (toggle on)
2. Set mode to "Verbose" for full request/response logging
3. Check debug output textarea in settings for errors
4. Listen to `remote-stt-error` event in frontend

### AI Replace
1. Listen to `ai-replace-error` event in `App.tsx` for error messages
2. Check console for "AI replace instruction:" and "AI replace selected text:" debug logs
3. Verify LLM provider is configured in Settings → Post-Processing

### Connector (Send to Extension)
1. Ensure Handy Connector extension is installed and bound to a tab
2. Test server manually: `curl http://127.0.0.1:63155/messages`
3. Check console for "Connector message sent" or error logs

### Send Screenshot to Extension
1. Listen to `screenshot-error` event in `App.tsx` — errors display for 5 seconds
2. Ensure ShareX (or configured tool) saves to the configured folder
3. Check "Require Recent Screenshot" is enabled to filter old files
4. Test screenshot tool manually: `"C:\Program Files\ShareX\ShareX.exe" -RectangleRegion`
5. Verify blob endpoint works: `curl http://127.0.0.1:63155/blob/{attId}`

### Selection Capture (Windows)
1. If selection capture fails, check Windows accessibility permissions
2. Test in different apps — some apps don't support standard selection APIs
3. Debug logs show "Selection copied in X ms (Y chars)"
