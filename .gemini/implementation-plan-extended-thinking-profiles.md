# Implementation Plan: Extended Thinking + Transcription Profiles

## Overview

Two features to implement:

1. **Extended Thinking (Reasoning)** - LLM reasoning/thinking tokens support for OpenRouter
2. **Transcription Profiles System** - Active profile switching with cycle shortcut

---

## Part 1: Extended Thinking / Reasoning

### 1.1 Backend (Rust)

#### `src-tauri/src/settings.rs`

- Add to `AppSettings`:

  ```rust
  // Post-Processing Extended Thinking
  pub post_process_reasoning_enabled: bool,      // default: false
  pub post_process_reasoning_budget: u32,        // default: 2048

  // AI Replace Extended Thinking
  pub ai_replace_reasoning_enabled: bool,        // default: false
  pub ai_replace_reasoning_budget: u32,          // default: 2048

  // Voice Commands LLM Extended Thinking
  pub voice_command_reasoning_enabled: bool,     // default: false
  pub voice_command_reasoning_budget: u32,       // default: 2048
  ```

#### `src-tauri/src/llm_client.rs`

- Add `ReasoningConfig` struct:
  ```rust
  pub struct ReasoningConfig {
      pub enabled: bool,
      pub max_tokens: u32,
  }
  ```
- Modify `ChatCompletionRequest` to include optional `reasoning` field
- Add `send_chat_completion_with_reasoning()` function:
  - If reasoning enabled: add `reasoning: { max_tokens: budget }` to request
  - Set `max_tokens = max(4000, reasoning_budget + 2000)`
  - **Fail-soft retry**: If 400 error, retry without reasoning silently
- Extract reasoning tokens from response, log via `log::info!`, don't include in returned content

#### `src-tauri/src/actions.rs`

- Update `post_process_transcription()` to use reasoning config from settings
- Update `ai_replace_with_llm()` to use reasoning config
- Update `generate_command_with_llm()` (Voice Commands) to use reasoning config

### 1.2 Frontend (React/TypeScript)

#### `src/stores/settingsStore.ts`

- Add setting updaters for all 6 new fields

#### `src/components/settings/post-processing/PostProcessingSettings.tsx`

- Add "Extended Thinking" section after model selector:
  - Toggle: "Enable Extended Thinking"
  - Number input: "Reasoning Token Budget" (only shown when toggle is on)
  - Default value: 2048, min: 1024

#### `src/components/settings/advanced/AiReplaceSettings.tsx`

- Add same "Extended Thinking" section

#### `src/components/settings/voice-commands/VoiceCommandSettings.tsx`

- Add "Extended Thinking" section in LLM Fallback area

#### `src/i18n/locales/en/translation.json`

- Add translation keys for all new UI elements

---

## Part 2: Transcription Profiles System

### 2.1 Backend (Rust)

#### `src-tauri/src/settings.rs`

- Modify `TranscriptionProfile`:
  ```rust
  pub struct TranscriptionProfile {
      pub id: String,
      pub name: String,
      pub language: String,
      pub translate_to_english: bool,
      pub description: String,
      pub system_prompt: String,
      pub include_in_cycle: bool,  // NEW: default true
  }
  ```
- Add to `AppSettings`:
  ```rust
  pub active_profile_id: String,                 // default: "default"
  pub profile_switch_overlay_enabled: bool,      // default: true
  ```

#### `src-tauri/src/shortcut.rs`

- Add new shortcut binding: `cycle_transcription_profile`
- Implement `CycleTranscriptionProfileAction`:
  - Check if recording/overlay active → block
  - Get next profile in cycle
  - Update `active_profile_id`
  - Show overlay if enabled
  - Emit event to frontend for UI update

#### `src-tauri/src/actions.rs`

- Modify `perform_transcription()`:
  - Get active profile
  - If "default" → use global settings
  - If custom → use profile's language, translate, system_prompt

#### `src-tauri/src/overlay.rs`

- Add `show_profile_switch_overlay(profile_name: &str)` function
- Short animation showing new active profile name

#### New Tauri Commands

- `get_active_profile() -> String`
- `set_active_profile(id: String) -> Result<(), String>`
- `cycle_to_next_profile() -> Result<String, String>` // returns new profile id

### 2.2 Frontend (React/TypeScript)

#### `src/stores/settingsStore.ts`

- Add `setActiveProfileId(id: string)` action
- Add `setProfileSwitchOverlayEnabled(enabled: bool)` action

#### `src/components/settings/TranscriptionProfiles.tsx` - Major Rewrite

- Add "Default" profile at top (non-deletable, uses global settings)
- Visual highlighting of active profile (glow/border)
- Each profile card:
  - Checkbox "Include in switching cycle" with tooltip
  - Shortcut assignment (optional)
  - "Set as Active" button (or click to activate)
- Add help text block at top explaining the system
- Compact layout

#### New Settings Elements

- Toggle: "Show overlay when switching profiles"
- Shortcut config for `cycle_transcription_profile`

#### Profile Switch Overlay

- Use existing overlay mechanism
- Show profile name with fade animation
- Auto-hide after ~1.5 seconds

#### `src/i18n/locales/en/translation.json`

- Add all new translation keys with good tooltip texts

---

## Implementation Order

### Phase 1: Extended Thinking (simpler)

1. Backend: settings.rs - add fields
2. Backend: llm_client.rs - add reasoning support + fail-soft
3. Backend: actions.rs - integrate reasoning config
4. Frontend: settingsStore.ts - add updaters
5. Frontend: UI components - add toggles and inputs
6. Frontend: translations

### Phase 2: Transcription Profiles

1. Backend: settings.rs - modify TranscriptionProfile, add active_profile_id
2. Backend: shortcut.rs - add cycle action
3. Backend: actions.rs - modify perform_transcription
4. Backend: overlay.rs - add profile switch overlay
5. Frontend: TranscriptionProfiles.tsx - major rewrite
6. Frontend: overlay component for profile switch
7. Frontend: translations and tooltips

---

## Files to Modify

### Backend

- `src-tauri/src/settings.rs`
- `src-tauri/src/llm_client.rs`
- `src-tauri/src/actions.rs`
- `src-tauri/src/shortcut.rs`
- `src-tauri/src/overlay.rs`
- `src-tauri/src/lib.rs` (register new commands)

### Frontend

- `src/stores/settingsStore.ts`
- `src/components/settings/post-processing/PostProcessingSettings.tsx`
- `src/components/settings/advanced/AiReplaceSettings.tsx`
- `src/components/settings/voice-commands/VoiceCommandSettings.tsx`
- `src/components/settings/TranscriptionProfiles.tsx`
- `src/overlay/RecordingOverlay.tsx` (or new component)
- `src/i18n/locales/en/translation.json`
- `src/bindings.ts` (auto-generated after Rust changes)

---

## Testing Checklist

### Extended Thinking

- [ ] Toggle enables/disables reasoning in requests
- [ ] Budget value is sent correctly
- [ ] Fail-soft retry works when model doesn't support reasoning
- [ ] Reasoning tokens logged but not in response
- [ ] Works for Post-Processing, AI Replace, Voice Commands

### Transcription Profiles

- [ ] Default profile exists and can't be deleted
- [ ] Active profile highlighted in UI
- [ ] Cycle shortcut works
- [ ] Cycle skips profiles with "include_in_cycle" = false
- [ ] Overlay shows on switch (when enabled)
- [ ] Can't switch during recording
- [ ] Direct profile shortcuts work
- [ ] Main Transcribe shortcut uses active profile
