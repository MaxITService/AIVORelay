# Deepgram Integration Plan (v1)

## Scope
- Integrate Deepgram as a first-class transcription provider (live + non-live).
- Reuse existing Soniox pipeline patterns where practical (audio capture, live preview, filters, cancellation, profile flow).
- Keep existing Soniox/OpenAI-compatible behavior unchanged.

## Status Snapshot (2026-03-05)
- Phase 1: Done.
- Phase 2: Done.
- Phase 3: Done.
- Phase 4: In progress (main wiring done; final UI cleanup/tuning remains).
- Phase 5: In progress (EN keys and code notes updated; final wording polish may remain).
- Validation: no automated build/tests run yet (per project rule).

## Rules
- No automated builds/tests run by agent unless explicitly requested.
- No commit unless explicitly requested.
- Prefer additive changes and fork-specific separation.

## Phase 1: Backend Foundation
1. Add new provider enum value: `remote_deepgram`.
2. Add Deepgram settings fields in `AppSettings`:
   - model, timeout, live_enabled, keepalive interval, finalize timeout, instant stop, interim_results, smart_format, diarize, endpointing.
3. Add secure API key slot in Windows Credential Manager:
   - `get/set/clear/has_deepgram_api_key`.
4. Add Tauri commands for Deepgram key management in `commands/remote_stt.rs`.

## Phase 2: Deepgram Managers
1. `deepgram_realtime.rs`:
   - WebSocket live session lifecycle.
   - Audio streaming (binary), control messages (`KeepAlive`, `Finalize`, `CloseStream`).
   - Result parsing (`Results`, `Metadata`), final/interim handling.
   - Callbacks for incremental output + live preview update.
   - finish/cancel timeout-safe flow.
2. `deepgram_stt.rs`:
   - Non-live transcription manager (operation ID + cancellation support).
   - Uses Deepgram streaming endpoint with full-clip flow and final transcript aggregation.

## Phase 3: Pipeline Wiring
1. Register managers in `managers/mod.rs` + `lib.rs` state.
2. Extend cancellation path in `utils.rs` to include Deepgram managers.
3. Extend `actions.rs`:
   - Provider branch in `perform_transcription_for_profile`.
   - Live start/stop handling for Deepgram alongside Soniox.
   - Reuse existing output filtering chain (custom words + filler filter).
4. Extend `commands/file_transcription.rs` with Deepgram provider branch.
5. Extend `shortcut.rs`:
   - provider parse accepts `remote_deepgram`.
   - add `change_deepgram_*` settings commands.
6. Register all new commands in `lib.rs` `collect_commands!`.

## Phase 4: Frontend Wiring
1. `RemoteSttSettings.tsx`:
   - add Deepgram provider option.
   - Deepgram API key flow (stored key indicator, save/clear).
   - Deepgram settings controls (model + live controls).
2. `settingsStore.ts`:
   - add invoke updaters for `deepgram_*` settings keys.
3. Provider checks:
   - `App.tsx`, `ModelSelector.tsx`, `TranslateToEnglish.tsx` (+ file transcription screen where needed).

## Phase 5: Texts & Notes
1. Add minimal EN i18n keys required for Deepgram UI labels.
2. Update `.AGENTS/code-notes.md` with new Deepgram files.

## Acceptance Criteria
- User can select `Deepgram` provider in settings.
- User can save/clear Deepgram API key.
- Live transcription works via Deepgram with stable stop/finalize behavior.
- Non-live transcription and file transcription work via Deepgram.
- Existing Soniox/OpenAI-compatible/local flows keep working unchanged.
