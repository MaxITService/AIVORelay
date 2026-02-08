# Recommended Integration (Fork-Native): Soniox for Cloud STT

## Goal
Integrate Soniox cloud transcription into AivoRelay without breaking existing fork features (`remote_stt`, Windows secure key storage, profile flow, onboarding flow, model selector behavior).

## Core Design Decision
Do not add a separate `TranscriptionMode`.  
Extend existing `TranscriptionProvider` instead.

Use:
- `local`
- `remote_openai_compatible` (existing)
- `remote_soniox` (new)

This keeps routing consistent with current fork architecture.

## Why Direct PR #700 Cherry-Pick Is Unsafe Here
- The fork already uses `transcription_provider` and `remote_stt`; adding parallel mode logic duplicates control paths.
- The PR stores Soniox API key in settings JSON; fork policy is Windows Credential Manager.
- The PR dependency edits conflict with fork runtime needs:
  - `tokio` must keep `net` and `sync` (connector server).
  - `reqwest` must keep `stream` (model download path).
- The PR targets `src-tauri/src/shortcut/mod.rs`, but this fork uses `src-tauri/src/shortcut.rs`.

## Backend Integration Plan

### 1) Settings Model
File: `src-tauri/src/settings.rs`
- Extend `TranscriptionProvider` with `RemoteSoniox` serialized as `remote_soniox`.
- Add Soniox settings block (model + timeout) under `AppSettings`.
- Keep API key out of JSON settings.

### 2) Secure API Key Storage
Files:
- `src-tauri/src/secure_keys.rs` (preferred for consistency), or
- `src-tauri/src/managers/remote_stt.rs` style functions (if keeping STT key handling local)

Add Soniox key helpers:
- `set_soniox_api_key`
- `get_soniox_api_key`
- `clear_soniox_api_key`
- `has_soniox_api_key`

Store as separate credential name from current remote OpenAI key.

### 3) Soniox Client / Manager
New file:
- `src-tauri/src/managers/soniox_stt.rs` (recommended)

Implement async Soniox flow:
- audio samples -> WAV
- upload file
- create transcription job
- poll status with timeout/backoff
- fetch transcript
- cleanup uploaded file

Return normalized text string and map errors to user-friendly messages.

### 4) Provider Routing in Live Transcription
File: `src-tauri/src/actions.rs`
- In transcription path, branch by provider:
  - `Local` -> existing local manager path
  - `RemoteOpenAiCompatible` -> existing remote manager path
  - `RemoteSoniox` -> new Soniox manager path
- Keep existing custom words/filtering behavior applied to Soniox output.
- Keep existing cancellation/overlay flow consistent.

### 5) Provider Routing in File Transcription
File: `src-tauri/src/commands/file_transcription.rs`
- Add Soniox branch when provider is `remote_soniox`.
- Reuse same profile language handling rules as remote provider flow.
- Keep subtitle fallback behavior explicit for cloud output when no timestamps are provided.

### 6) Tauri Commands Exposure
Files:
- `src-tauri/src/commands/remote_stt.rs` (or dedicated Soniox command module)
- `src-tauri/src/lib.rs`
- `src-tauri/src/shortcut.rs`

Expose commands for:
- Soniox API key set/clear/status
- Soniox model set
- Soniox timeout set/get

Register these in `lib.rs` command collector.

## Frontend Integration Plan

### 1) Reuse Existing Remote STT Settings UI
Primary file:
- `src/components/settings/remote-stt/RemoteSttSettings.tsx`

Add provider option:
- `remote_soniox`

Behavior:
- If `remote_openai_compatible`: show Base URL + Model ID + API Key + debug fields.
- If `remote_soniox`: show Soniox model + timeout + API key controls (hide OpenAI base URL).

### 2) Settings Store Wiring
File: `src/stores/settingsStore.ts`
- Support `transcription_provider = remote_soniox`.
- Add update handlers for Soniox model and timeout.
- Add Soniox key actions (set/clear/has).

### 3) Bindings
File: `src/bindings.ts`
- Regenerate after Rust command/type updates.
- Ensure new provider union and Soniox command wrappers are present.

### 4) Provider-Aware UX Updates
Files:
- `src/components/settings/TranslateToEnglish.tsx`
- `src/components/settings/TranscriptionSystemPrompt.tsx`
- `src/components/settings/TranscriptionProfiles.tsx`
- `src/components/model-selector/ModelSelector.tsx`
- `src/components/onboarding/Onboarding.tsx`
- `src/components/onboarding/RemoteSttWizard.tsx` (or Soniox variant)

Required behavior:
- Disable translation toggle for Soniox if Soniox path does not support equivalent translation endpoint.
- Disable STT prompt UI for Soniox if Soniox API path does not accept prompt-like parameter.
- Ensure model selector and onboarding correctly reflect 3 providers (not 2).

### 5) Localization
File:
- `src/i18n/locales/en/translation.json` (then propagate to other locales)

Add:
- provider label for Soniox
- Soniox field labels/hints/errors

## Suggested Implementation Order (Low-Risk)
1. Add backend provider enum + Soniox settings + key storage helpers.
2. Add Soniox manager/client and wire backend routing (live + file transcription).
3. Add minimal UI provider selection and Soniox credential/model/timeout fields.
4. Update translation/prompt/profile/model-selector/onboarding provider logic.
5. Regenerate bindings and update locale strings.

## Acceptance Criteria
- User can select `remote_soniox` and transcribe via Soniox.
- Soniox API key is stored in Windows Credential Manager, not plaintext settings.
- Existing `remote_openai_compatible` behavior remains unchanged.
- Existing `local` behavior remains unchanged.
- No regressions in connector/model-download paths from dependency feature changes.

## Explicit Non-Goals for Initial Pass
- Replacing existing remote OpenAI-compatible provider.
- Refactoring all remote providers into one generic abstraction in the first pass.
- Introducing unrelated Apple Intelligence build changes while integrating Soniox.

## Phase 1 (MVP, Non-Streaming)

### Phase 1 Objective
Ship `remote_soniox` as a working cloud STT provider with final-text output only, while keeping existing `local` and `remote_openai_compatible` paths stable.

### Phase 1 Scope (In)
- Provider enum and settings support for `remote_soniox`.
- Soniox API key management via Windows Credential Manager.
- Soniox transcription path for:
  - live shortcut transcription (`actions.rs`)
  - file transcription (`file_transcription.rs`)
- Minimal settings UI to select Soniox and configure key/model/timeout.
- Bindings update for all new commands/types.
- English locale strings for Soniox labels/hints/errors.

### Phase 1 Scope (Out)
- Partial result streaming or live token-by-token UI updates.
- Soniox-specific onboarding wizard redesign.
- Broad refactor of remote STT abstractions.
- Non-English locale rollout (can follow after MVP).

### Phase 1 Implementation Checklist

#### A) Backend data model and commands
Files:
- `src-tauri/src/settings.rs`
- `src-tauri/src/shortcut.rs`
- `src-tauri/src/lib.rs`

Tasks:
- Add `TranscriptionProvider::RemoteSoniox` serialized as `remote_soniox`.
- Add Soniox settings fields in `AppSettings`:
  - `soniox_model`
  - `soniox_timeout_seconds`
- Add defaults and migration-safe serde defaults.
- Add commands to set/get Soniox model and timeout.
- Extend `change_transcription_provider_setting` to accept `remote_soniox`.
- Register new commands in command collector in `lib.rs`.

#### B) Secure key storage
Files:
- `src-tauri/src/secure_keys.rs` (preferred)
- `src-tauri/src/commands/remote_stt.rs` or new `src-tauri/src/commands/soniox_stt.rs`

Tasks:
- Add Soniox key helpers:
  - `set_soniox_api_key`
  - `get_soniox_api_key`
  - `clear_soniox_api_key`
  - `has_soniox_api_key`
- Expose Tauri commands:
  - `soniox_has_api_key`
  - `soniox_set_api_key`
  - `soniox_clear_api_key`

#### C) Soniox transcription client
Files:
- `src-tauri/src/managers/soniox_stt.rs` (new)
- `src-tauri/src/managers/mod.rs`

Tasks:
- Implement non-streaming async workflow:
  - encode WAV
  - upload file
  - create transcription job
  - poll status with timeout/backoff
  - fetch transcript
  - cleanup uploaded file
- Return `Result<String>` and normalize error messages for UI overlay.

#### D) Transcription routing integration
Files:
- `src-tauri/src/actions.rs`
- `src-tauri/src/commands/file_transcription.rs`

Tasks:
- In live transcription provider branch, add Soniox path.
- In file transcription provider branch, add Soniox path.
- Keep existing post-processing/custom-words/filler filtering behavior consistent.
- Keep cancel/error overlay behavior aligned with current remote flow.

#### E) Frontend minimal integration
Files:
- `src/components/settings/remote-stt/RemoteSttSettings.tsx`
- `src/stores/settingsStore.ts`
- `src/bindings.ts`
- `src/i18n/locales/en/translation.json`

Tasks:
- Add provider option `remote_soniox`.
- For Soniox provider, show:
  - API key controls
  - model
  - timeout
- Hide OpenAI-specific base URL/debug controls when Soniox is selected.
- Wire store actions for Soniox fields and key commands.
- Regenerate bindings after Rust changes.

#### F) Provider-aware guardrails
Files:
- `src/components/settings/TranslateToEnglish.tsx`
- `src/components/settings/TranscriptionSystemPrompt.tsx`
- `src/components/settings/TranscriptionProfiles.tsx`

Tasks:
- Disable translation toggle when provider is Soniox (unless later supported).
- Disable STT prompt UI for Soniox (unless API prompt capability is added later).
- Keep profile UI behavior coherent when active provider is Soniox.

### Phase 1 Done Criteria
- `remote_soniox` can be selected in settings.
- Soniox API key is stored and read from Windows Credential Manager.
- Live shortcut transcription works and produces final text via Soniox.
- File transcription works via Soniox and returns final text (subtitle fallback still works).
- Existing `local` and `remote_openai_compatible` continue to function unchanged.
- No dependency feature regressions (`tokio net/sync`, `reqwest stream` remain intact).
