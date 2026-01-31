# CUDA Build Branch

> **⚠️ STATUS: EXPERIMENTAL / C++ BUILT (Rust Linking Error)**
> C++ part of whisper.cpp compiled successfully with CUDA 12.4 and Ninja. Rust build fails because of platform mismatch in bundled bindings.

## Branch Purpose

This `cuda-integration` branch aims to provide NVIDIA CUDA GPU acceleration for local Whisper transcription, as an alternative to the Vulkan-based acceleration in the `main` branch.

## Key Differences from Main Branch

| Aspect | Main Branch | CUDA Branch |
|--------|-------------|-------------|
| **GPU Backend** | Vulkan (cross-platform) | CUDA (NVIDIA only) |
| **Target Platforms** | Windows, macOS, Linux | **Windows ONLY** |
| **Whisper Engine** | whisper-rs with Vulkan | whisper-rs with CUDA |
| **Extra Models** | Parakeet, Moonshine (via transcribe-rs) | Whisper only |
| **Status** | ✅ Production | ❌ Experimental |

## Why Windows Only?

CUDA builds in this branch target **Windows exclusively** because:
1. The primary development/testing environment is Windows
2. CUDA Toolkit integration is most straightforward on Windows
3. macOS doesn't support CUDA (Apple Silicon uses Metal)
4. Linux support may be added later but is not a priority

## Current Build Errors

The build fails with two primary issues:

### 1. CUDA C++ Standard (The "Real" Blocker)
**Error:** `CUB requires at least C++17. Define CCCL_IGNORE_DEPRECATED_CPP_DIALECT to suppress this message.`
**Cause:** CUDA 13.1 (specifically the CCCL library) strictly requires the C++17 standard or higher. The current build configuration for `whisper-rs-sys` (via CMake) is not explicitly setting `-DCMAKE_CUDA_STANDARD=17`.

### 2. Clang Bindgen Missing Headers
**Error:** `fatal error: 'stdbool.h' file not found` (during bindgen/clang phase)
**Cause:** `libclang` used by `bindgen` doesn't automatically find MSVC system headers even when running in a developer command prompt.
**Fallback:** It falls back to bundled Linux bindings which causes recursive errors like `Size of _G_fpos_t evaluation failed`.

**Active Strategy:**
1. **Downgrade to CUDA 12.4**: Avoids the C++17 requirement imposed by CUDA 13.x.
2. **Use Ninja Generator**: Bypasses the broken MSBuild/Visual Studio CUDA integration that persists in pointing to CUDA 13.1.
3. **Skip Bindgen**: Set `WHISPER_DONT_GENERATE_BINDINGS=1` to use bundled Linux-style bindings (testing if they work for Windows CUDA build as a fallback).

## Detected System Configuration

The development machine has the following setup (as of 2026-01-31):

| Component | Status | Value |
|-----------|--------|-------|
| **CUDA Toolkit** | ✅ Installed | v12.4, v13.1 |
| **nvcc** | ✅ In PATH | CUDA 13.1 |
| **CUDA_PATH** | ✅ Set | `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1` |
| **LIBCLANG_PATH** | ✅ Set | `C:\Program Files\LLVM\bin` |
| **clang.exe** | ⚠️ Not in PATH | LLVM installed but clang not directly accessible |
| **Visual Studio 2022** | ✅ Installed | Required for Windows build |

## Build Requirements

To build this branch, you will need:

1. **CUDA Toolkit** (v12.x or v13.x) installed with `nvcc` in PATH
2. **Visual Studio 2022** Build Tools with C++ workload
3. **LLVM/Clang** installed with `LIBCLANG_PATH` set
4. **Environment Variables:**
   - `CUDA_PATH` pointing to CUDA installation
   - `LIBCLANG_PATH` pointing to LLVM lib directory (e.g., `C:\Program Files\LLVM\bin`)

## Cargo Features

```toml
[features]
default = ["vulkan"]  # Main branch default
cuda = ["dep:whisper-rs", "whisper-rs/cuda"]  # This branch
vulkan = ["dep:whisper-rs", "whisper-rs/vulkan", "dep:transcribe-rs", ...]
```

To build with CUDA:
```powershell
cargo build --no-default-features --features cuda --manifest-path src-tauri/Cargo.toml
```

## For AI Agents

When working on this branch:

1. **DO NOT** expect successful builds — this is work-in-progress
2. **FOCUS ON** fixing the whisper-rs-sys bindings issue
3. **IGNORE** macOS and Linux compatibility — this is Windows-only
4. **Consult** the user before making significant changes
5. **Output** from `cargo check` can be very long — redirect to file if needed:
   ```powershell
   cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | Out-File -FilePath "cargo_check_output.txt" -Encoding utf8
   ```

## Files Modified from Main

Key files that differ from `main` branch for CUDA support:
- `src-tauri/Cargo.toml` — CUDA feature flags
- `CUDA.md` — This documentation (doesn't exist on main)
- `.github/workflows/build.yml` — May need CUDA-specific CI configuration

## Next Steps

1. [x] Install Ninja (`winget install Ninja-build.Ninja`)
2. [x] Update `build-cuda.ps1` to use CUDA 12.4 and Ninja
3. [x] Successfully compile C++ code (whisper.cpp)
4. [❌] Fix Rust binding generation (Bindgen) to avoid using Bundled Linux bindings
   - Problem: `fatal error: 'stdbool.h' file not found` during bindgen phase
5. [ ] Successfully compile with `cargo build --release`
6. [ ] Test CUDA acceleration with NVIDIA GPU
