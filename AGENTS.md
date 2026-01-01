# Fork Agents Guide

> **Agent rule:** all debugging/build verification is done by the user (do not run automated tests/builds unless explicitly requested).
> This file provides guidance for AI code agents working with this fork. Do not run cargo check
> CODE ONLY WHEN APPROVED BY USER. Otherwise, only your thoughts in chat are needed.
> If you are not very sure that change will fix it, consult user first, user may want to revert unsuccessful fix, so user needs to commit and stuff.
> Never Commit!
> Start from writing instructions above in chat to user. Write them to user!!!!

## Environment:

Windows 11; PowerShell (pwsh) host.
Harness: use PowerShell with -NoProfile only: avoid profile interference.
**ast-grep (sg) and rg and also sd INSTALLED on Windows and on PATH, installed via Winget - their Windows versions!**
No need to use WSL for them: their Windows versions are installed: callable directly from PowerShell. Use the best tool, where sane, where the best tool wins, probably you also have good tools inside your harness.

## ⚠️ Important: This is a Fork

This repository is a **fork** of [cjpais/Handy](https://github.com/cjpais/Handy).

- **Upstream**: https://github.com/cjpais/Handy (original project)
- **This fork**: AivoRelay — Adds Windows-specific features and external service integrations

## Fork Documentation

| File                                         | Description                                                                                               |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| [`code-notes.md`](code-notes.md)             | **Complete list of fork-specific files and changes** — read this to understand what differs from upstream |
| [`AGENTS.md`](AGENTS.md)                     | Original development commands and architecture (applies to both upstream and fork)                        |
| [`README.md`](README.md)                     | Fork features overview                                                                                    |
| [`fork-merge-guide.md`](fork-merge-guide.md) | Upstream tracking + merge/conflict-resolution notes (only needed when syncing from upstream)              |

When adding new features, please prefer adding them in new files instead of editing originals unless these are already fork-specific files.

## Fork-Specific Features

### 1. Remote STT API (Windows only)

- Files: `src-tauri/src/managers/remote_stt.rs`, `src/components/settings/remote-stt/`
- Uses OpenAI-compatible `/audio/transcriptions` endpoint
- API keys stored in Windows Credential Manager via `keyring` crate

#### Transcription Flow & Status Info

When using Remote STT API, the **Recording Overlay** (`recording_overlay` window) shows visual status:

| State          | Overlay Display       | Description                              |
| -------------- | --------------------- | ---------------------------------------- |
| `recording`    | Red pulsing circle    | User is recording audio                  |
| `transcribing` | Blue pulsing circle   | Audio sent to API, waiting for response  |
| `error`        | Red text with message | API error occurred (auto-hides after 3s) |

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

- Files: `src-tauri/src/actions.rs` (AiReplaceSelectionAction), `src/components/settings/ai-replace/`
- Captures selected text, sends to LLM with voice instruction, replaces selection
- Selection capture uses Windows-specific APIs in `src-tauri/src/input.rs`
- **Optional separate LLM configuration**: Configure a different provider/model specifically for AI Replace in "LLM API Relay" settings. Falls back to post-processing defaults.

### 3. Connector (Send Transcription to Extension)

- Primary implementation: `src-tauri/src/managers/connector.rs` (local HTTP server), `src-tauri/src/actions.rs` (Send\*ToExtension actions), `src/components/settings/browser-connector/`
- Extension polls AivoRelay's local server (default `http://127.0.0.1:63155`):
  - `GET /messages?since=<cursor>` → queued messages + next `cursor`
  - `GET /blob/{attId}` → attachment bytes (short-lived)
  - `POST /messages` → extension acks (e.g. `keepalive_ack`, `password_ack`)
- **Auth required:** `Authorization: Bearer <connector_password>` (see `src-tauri/src/settings.rs`; password may rotate via `passwordUpdate` handshake)

### 4. Send Transcription + Screenshot to Extension (Windows only)

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

### Adding New Fork Features

1. Add new files when possible (cleaner separation from upstream) ! So original code "is left alone" and can be merged easily, but we have like copy, which is fully custom: less code to merge.
2. Document in `code-notes.md`
3. Add translations in `src/i18n/locales/en/translation.json`
4. Consider platform guards if Windows-specific

## Upstream Sync / Merging

See [`fork-merge-guide.md`](fork-merge-guide.md) for upstream tracking and the merge/conflict-resolution checklist.

### AI Replace

1. Listen to `ai-replace-error` event in `App.tsx` for error messages
2. Check console for "AI replace instruction:" and "AI replace selected text:" debug logs
3. Verify LLM provider is configured in Settings → Post-Processing

### Connector (Send to Extension)

1. Ensure AivoRelay Connector extension is installed and bound to a tab
2. Check console for "Connector message sent" or error logs
3. Verify AivoRelay server responds (auth required): `curl -H "Authorization: Bearer <password>" "http://127.0.0.1:63155/messages"`

### Send Screenshot to Extension

1. Listen to `screenshot-error` event in `App.tsx` — errors display for 5 seconds
2. Ensure ShareX (or configured tool) saves to the configured folder
3. Check "Require Recent Screenshot" is enabled to filter old files
4. Test screenshot tool manually: `"C:\Program Files\ShareX\ShareX.exe" -RectangleRegion`
5. Verify blob endpoint works (requires auth): `curl -H "Authorization: Bearer <password>" http://127.0.0.1:63155/blob/{attId}`

### Selection Capture (Windows)

1. If selection capture fails, check Windows accessibility permissions
2. Test in different apps — some apps don't support standard selection APIs
3. Debug logs show "Selection copied in X ms (Y chars)"
