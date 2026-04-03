# CUDA Main Diff Manifest

This note documents the current `cuda-integration` branch as a content diff against `main`.

Baseline used for this note:

- `main` content baseline: `4d2750b5`
- note date: `2026-04-03`
- this note describes the current files on disk; it is not a git-ancestry claim

Primary companion docs:

- [[.AGENTS/cuda-docs-branch-overview|cuda-docs-branch-overview.md]]
- [[.AGENTS/cuda-docs-build-runbook|cuda-docs-build-runbook.md]]
- [[.AGENTS/cuda-docs-sync-maintenance|cuda-docs-sync-maintenance.md]]

## Branch Rule

- ordinary app code should stay close to `main`
- only CUDA/build/release/runtime-specific changes should remain branch-local unless the user explicitly approves a wider CUDA divergence

## Files Different From `main`

| File | How it differs from `main` |
| --- | --- |
| `.AGENTS/cuda-docs-index.md` | Branch-local entry point for CUDA docs. |
| `.AGENTS/cuda-docs-branch-overview.md` | Branch-local overview of CUDA intent, dependency model, and runtime assumptions. |
| `.AGENTS/cuda-docs-build-runbook.md` | Branch-local build, toolchain, bindings, and verification rules. |
| `.AGENTS/cuda-docs-model-runtime-notes.md` | Branch-local model/runtime behavior, including Cohere FP16/FP32 notes. |
| `.AGENTS/cuda-docs-main-diff-manifest.md` | Authoritative branch-local manifest of `cuda-integration -> main` differences. |
| `.AGENTS/cuda-docs-release-runbook.md` | CUDA release rules and workflow notes. |
| `.AGENTS/cuda-docs-sync-maintenance.md` | CUDA sync-log and cursor maintenance note. |
| `.AGENTS/branch-propagation-log.md` | Records propagated `main` commits on top of the CUDA branch-local layer. |
| `.AGENTS/branching-status.md` | Keeps the current `main` sync cursor for `cuda-integration`. |
| `.cargo/config.toml` | Uses the short target dir `C:/aivorelay-cuda` and the branch-local Windows linker setting. |
| `.github/workflows/cuda-release.yml` | Branch-local Windows CUDA release pipeline. |
| `AGENTS.md` | Short CUDA branch entry file pointing to the prefixed CUDA docs set. |
| `README.md` | Keeps the CUDA Edition release mention and now points readers to the CUDA docs index. |
| `build-cuda.ps1` | Branch-local CUDA build/dev entrypoint. |
| `src-tauri/Cargo.toml` | Uses the CUDA/local-fork dependency model instead of `main`'s default Windows dependency layout. |
| `src-tauri/Cargo.lock` | Resolves against the local CUDA dependency graph instead of `main`'s default lock layout. |
| `src-tauri/src/managers/model.rs` | CUDA branch adds Cohere FP16/FP32 catalog entries, local folder detection, tokenizer-to-tokens generation, and multi-file ONNX download wiring. |
| `src-tauri/src/managers/transcription.rs` | CUDA branch adds Cohere quantization auto-detection for Int8, FP16, and FP32. |
| `src/i18n/locales/en/translation.json` | Adds user-facing labels and descriptions for CUDA-branch Cohere FP16/FP32 variants. |
| `src/i18n/locales/ru/translation.json` | Adds user-facing labels and descriptions for CUDA-branch Cohere FP16/FP32 variants. |
