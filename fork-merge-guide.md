# Upstream Sync & Merge Guide

This file contains the upstream-tracking + merge/conflict-resolution notes for this fork. Only read this when syncing/merging from upstream.

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

We will cherry-pick commits from upstream to our branches. ONE BY ONE. Ask user for guidance if needed. Ask user if next commit is good to cherry-pick, then do it, resolve conflicts, report back to user.
Do not cherry-pick commits that are not for Windows! 
When merging upstream changes, these files will likely have conflicts. Here's how to resolve them:

### High-Conflict Files (Modified by Fork)

#### `src-tauri/src/settings.rs`
**Our additions (MUST KEEP):**
- `TranscriptionProvider` enum (`Local`, `RemoteOpenAiCompatible`)
- `RemoteSttSettings` struct (base_url, model_id, debug_mode, debug_capture)
- Fields in `AppSettings`: `transcription_provider`, `remote_stt`, `ai_replace_*`, `connector_*`, `screenshot_*`

**Merge strategy:** Keep all our additions. Accept upstream changes to other fields. If upstream adds new settings, add them alongside ours. Think of best soltion, ask user if there is only one thing that we can keep. Below are just recommendations:

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
- `src-tauri/src/managers/connector.rs` — Main connector module (HTTP server for extension)
- `src-tauri/src/commands/connector.rs` — Tauri commands for connector
- `src-tauri/src/managers/remote_stt.rs`
- `src-tauri/src/commands/remote_stt.rs`
- `src-tauri/src/plus_overlay_state.rs` — Extended overlay states for error display
- `src/components/settings/remote-stt/RemoteSttSettings.tsx`
- `src/components/settings/advanced/AiReplaceSettings.tsx`
- `src/components/settings/browser-connector/ConnectorStatus.tsx` — Extension status indicator
- `src/components/icons/SendingIcon.tsx` — Icon for "sending" overlay state
- `src/overlay/plus_overlay_states.ts` — TypeScript types for extended overlay

### After Merge Checklist

1.  Add latest upstream commit SHA and  message here in this file, so we know where we are diverged from upstream.
   
