# CUDA Branch Overview

This note describes what `cuda-integration` is for and what stays branch-local here.

## Purpose

- Keep normal app behavior close to `main`.
- Keep NVIDIA/CUDA-specific build, dependency, and runtime behavior in this branch.
- Keep branch-local documentation in `.AGENTS` instead of spreading it between root and agent notes.

## Start Here

- For build/dev commands, read [[.AGENTS/cuda-docs-build-runbook|cuda-docs-build-runbook.md]].
- For local model and runtime behavior, read [[.AGENTS/cuda-docs-model-runtime-notes|cuda-docs-model-runtime-notes.md]].
- For the exact branch-local diff against `main`, read [[.AGENTS/cuda-docs-main-diff-manifest|cuda-docs-main-diff-manifest.md]].

## Dependency Model

- `src-tauri/Cargo.toml` uses `whisper-cpp` + `onnx`.
- On Windows, this branch additionally enables `whisper-cuda` + `ort-cuda`.
- The branch uses local dependency forks in `C:\Code\AIVORelay-deps`.
- Local builds use [[build-cuda.ps1]] instead of the default `main` flow.

## Branch-Local Runtime Notes

- This branch is aimed at NVIDIA/CUDA-capable Windows systems.
- Runtime still expects the required NVIDIA/CUDA libraries to exist on the target system.
- The branch now carries extra local Cohere variants beyond `main`:
  - `Cohere FP16`
  - `Cohere FP32`
- Those Cohere variants are documented as CUDA/NVIDIA-oriented options in this branch, with much heavier VRAM/RAM usage than Int8.
