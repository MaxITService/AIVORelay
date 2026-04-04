# CUDA Model And Runtime Notes

This note keeps branch-local model behavior and runtime assumptions that do not fit well inside the build runbook.

## Runtime Assumptions

- This branch builds a CUDA-enabled executable for NVIDIA-capable Windows systems.
- Runtime still expects the required NVIDIA/CUDA libraries to be available on the target system.
- The portable CUDA release is not documented as a fully self-contained universal Windows package.

## Local Cohere Variants In This Branch

The CUDA branch currently carries these local Cohere variants:

- `cohere-int8`
- `cohere-fp16`
- `cohere-fp32`

Intent in this branch:

- `cohere-int8` remains the lighter packaged local option.
- `cohere-fp16` is a CUDA/NVIDIA-oriented higher-precision option.
- `cohere-fp32` is the heaviest local Cohere option and is documented as CUDA/NVIDIA-oriented.

Practical expectations:

- FP16 and FP32 use much more VRAM/RAM than Int8.
- FP16 is the more realistic first choice for local GPU use.
- FP32 is the largest and slowest local Cohere variant in this branch.

## Local Folder Behavior

The branch-local Cohere loader now understands local Cohere directories with Int8, FP16, or FP32 ONNX layouts.

Current behavior:

- already-present local Cohere folders can be auto-detected
- the loader auto-detects Int8, FP16, or FP32 from the files in the model directory
- if a Cohere folder contains `tokenizer.json` but no `tokens.txt`, the app generates `tokens.txt` locally before loading

For local CUDA branch troubleshooting:

- prefer the CUDA debug executable build when the release build hides model load, file layout, ONNX session, or download-path failures

## Download Sources

For branch-local Cohere FP16 and FP32 support, the app currently downloads from:

- [onnx-community/cohere-transcribe-03-2026-ONNX](https://huggingface.co/onnx-community/cohere-transcribe-03-2026-ONNX)

Base model reference:

- [CohereLabs/cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026)

The FP16/FP32 flow is a multi-file ONNX download, not the older single-archive model path used by some other bundled models.
