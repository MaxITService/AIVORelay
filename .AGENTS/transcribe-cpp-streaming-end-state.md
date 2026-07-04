# transcribe.cpp / Native Streaming End State
Branch tags: #branch/main

This note records the end state after the upstream Handy transcribe.cpp / streaming intake.

## Source Context

- Upstream reference commit: `31d8fc2` / `31d8fc24`, "introduce transcribe.cpp".
- The upstream commit was a large mixed commit: backend, native streaming, catalog, model capability probing, UI/settings churn, release notes, translations, and build/CI work.
- The fork did not cherry-pick it whole. The fork manually adapted the runtime pieces that matter for AIVORelay and preserved fork-specific preview, overlay, model selector, and settings architecture.

## Local Commit Shape

Current main-line commits that contain the adapted end state:

- `d56c271d` `feat(models): add transcribe.cpp catalog and perf cli`
  - Main transcribe.cpp backend/catalog/native-streaming adaptation.
  - Includes follow-up polish that was autosquashed into the adaptation instead of left as a separate cleanup commit.
- `81fff576` `feat(preview): generalize live preview streaming updates`
  - Fork preview naming/generalization work around live preview events and overlay helpers.
- `daad8095` `fix(audio): preserve unicode VAD resource paths`
- `46dc2130` `fix(audio): reduce microphone start latency`
- `8e8c77de` `fix(windows): bundle VC runtime app-local`
- `d023b2c2` `chore(deps): bump transcribe-cpp to 0.1.1`

These hashes may change if local history is rebased before publishing; use commit messages and file content as the durable reference.

## Runtime End State

- `EngineType::TranscribeCpp` is a first-class local engine.
- Catalog-backed GGUF models use the new JSON catalog while the UI keeps the old friendly model selector shape.
- Batch transcription uses `transcribe_cpp::Session::run()`.
- Native realtime preview uses `transcribe_cpp::Session::stream()` with a worker command loop:
  - `Feed(Vec<f32>)`
  - `Finalize(...)`
  - `Cancel`
- Streaming feeds PCM frames into the native stream instead of repeatedly re-transcribing a growing audio buffer.
- Native stream finalization returns the selected/effective language with the final text so filler-word filtering does not accidentally use the global settings language.
- Legacy local preview auto-flush remains as a fallback for non-native-streaming local models and is intentionally marked legacy / not recommended in app copy.

## Model Capability End State

- `ModelInfo` now carries:
  - `supports_streaming`
  - `supports_translation`
  - `supports_language_detection`
  - `supported_languages`
- Catalog models read `capabilities.streaming`, `capabilities.translate`, and `capabilities.lang_detect`.
- Custom local `.bin` and `.gguf` files are discovered as transcribe.cpp models.
- GGUF headers are probed through:
  - `src-tauri/src/managers/gguf_meta.rs`
  - `src-tauri/src/managers/model_capabilities.rs`
- Hugging Face cache GGUF discovery is supported.
- Runtime model capabilities are reconciled after loading a transcribe.cpp model because the compiled backend/model can know more than the static catalog.
- `rescan_local_models` exists so the frontend can refresh discovered local models without restarting.

## Language End State

- The fork preserves profile language / translate overrides instead of blindly reading global settings.
- `model::effective_language(...)` normalizes requested language against model-supported languages and model language-detection support.
- Chinese script UI intent (`zh-Hans`, `zh-Hant`) is preserved for the selector while recognition can map to base `zh` when required by the backend.
- For models without language detection, `auto` falls back to English when available, otherwise to the first supported language.

## Important Non-Goals

- Do not import upstream `ModelSource` wholesale unless there is a separate design decision to replace the fork's model architecture.
- Do not commit raw upstream reference dumps under `src-tauri/src/...`; temporary upstream comparison files belong outside production source.
- Do not replace the fork preview/overlay pipeline with upstream overlay code. Adapt native streaming into the fork pipeline.
- Do not treat "transcribe.cpp works for batch files" as proof that native streaming is adapted; those are separate behaviors.

## Primary Files

- `src-tauri/src/managers/transcription.rs`
- `src-tauri/src/managers/model.rs`
- `src-tauri/src/managers/gguf_meta.rs`
- `src-tauri/src/managers/model_capabilities.rs`
- `src-tauri/src/actions.rs`
- `src-tauri/src/catalog/catalog.json`
- `src-tauri/src/catalog/mod.rs`
- `src-tauri/src/commands/models.rs`
- `src/bindings.ts`
