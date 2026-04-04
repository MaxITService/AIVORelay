# CUDA Model And Runtime Notes

This note keeps branch-local model behavior and runtime assumptions that do not fit well inside the build runbook.

## Runtime Assumptions

- This branch builds a CUDA-enabled executable for NVIDIA-capable Windows systems.
- Runtime still expects the required NVIDIA/CUDA libraries to be available on the target system.
- The portable CUDA release is not documented as a fully self-contained universal Windows package.

## Local Cohere Variants In This Branch

The app-facing CUDA branch now ships two distinct Cohere paths:

- `cohere-int8`
- `cohere-fp32`

Important split:

- `cohere-int8` stays on the packaged legacy Cohere Int8 runtime contract
- `cohere-fp32` uses a separate HF-style split-graph runtime contract
- `cohere-fp16` is still not shipped; FP16 graph surgery is unresolved

## Local Folder Behavior

Current app behavior:

- already-present local legacy Cohere Int8 folders can be auto-detected
- already-present local split-graph HF-style Cohere FP32 folders can also be auto-detected
- if a supported legacy Cohere folder contains `tokenizer.json` but no `tokens.txt`, the app generates `tokens.txt` locally before loading
- the same `tokenizer.json -> tokens.txt` generation is used for HF-style Cohere folders

For local CUDA branch troubleshooting:

- prefer the CUDA debug executable build when the release build hides model load, file layout, ONNX session, or download-path failures

## HF-Style FP32 Path

The separate HF-style Cohere GPU experiments are documented locally in:

- [[.AGENTS/.UNTRACKED/cohere-hf-gpu-runner-notes|.AGENTS/.UNTRACKED/cohere-hf-gpu-runner-notes.md]]

That work now backs the app-facing `cohere-fp32` model path:

- graph/export: `eschmidbauer/cohere-transcribe-03-2026-onnx`
- processor/tokenizer assets: `onnx-community/cohere-transcribe-03-2026-ONNX`
- download flow is multi-file and external-source backed

The key architectural point is unchanged:

- HF-style Cohere FP32 is a different runtime contract from the packaged Int8 Cohere backend
- future FP16 support should reuse the same HF-style backend only after graph surgery succeeds
