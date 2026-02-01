# CUDA Build Branch

> **STATUS: WORKING** (2026-02-01)
> CUDA build compiles successfully with CUDA 12.4 + patched whisper-rs.

## Quick Start: How to Build

### Prerequisites

1. **CUDA Toolkit 12.4** (NOT 13.x - see why below)
   ```
   C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4
   ```

2. **Visual Studio 2022** with C++ workload
   - Desktop development with C++
   - MSVC v143 build tools

3. **LLVM/Clang** (for bindgen)
   - Download from https://releases.llvm.org/
   - Install to `C:\Program Files\LLVM`

4. **Ninja** build system
   ```powershell
   winget install Ninja-build.Ninja
   ```

5. **Patched whisper-rs fork** at `C:\Code\experiments\whisper-rs-test`
   - Branch: `cuda-windows-patches`
   - Contains fixes for Windows CUDA build and GGML linking
   - See "Fork Patches" section below

### Build Commands

```powershell
# From project root (c:\Code\Released Software\AIVORelay)

# Option 1: Just check if it compiles
.\build-cuda.ps1 -Check

# Option 2: Full Tauri Bundle (Recommended for first use)
# This solves the "localhost refused to connect" error by embedding assets
.\build-cuda.ps1 -Full

# Option 3: Tauri Dev Mode (For development)
# This launches the app with HMR and CUDA enabled
.\build-cuda.ps1 -Dev

# Option 4: Just build the binary (no bundling)
.\build-cuda.ps1 -Build

# Option 5: Clean rebuild
.\build-cuda.ps1 -Clean -Full
```

### Manual Build (without script)

```powershell
# Load VS environment
cmd /c '"C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" -arch=x64 && set' |
    Where-Object { $_ -match '^(.+?)=(.*)$' } |
    ForEach-Object { Set-Item "Env:$($Matches[1])" $Matches[2] }

# Set CUDA 12.4 (not 13.x!)
$env:CUDA_PATH = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4"
$env:CMAKE_GENERATOR = "Ninja"

# Set bindgen paths
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"

# Build
cargo build --manifest-path src-tauri/Cargo.toml --release --no-default-features --features cuda
```

---

## Key Differences from Main Branch (Vulkan)

| Aspect | Main Branch | CUDA Branch |
|--------|-------------|-------------|
| **GPU Backend** | Vulkan (cross-platform) | CUDA (NVIDIA only) |
| **Target Platform** | Windows, macOS, Linux | **Windows ONLY** |
| **CUDA Toolkit** | Not required | Required (v12.4) |
| **Transcription Models** | Whisper, Parakeet, Moonshine | **Whisper only** |
| **transcribe-rs** | Included | NOT included |
| **whisper-rs** | crates.io version | Patched local fork |

### Why No transcribe-rs?

The `cuda` feature excludes `transcribe-rs` because:
1. `transcribe-rs` uses Vulkan for GPU acceleration
2. Mixing CUDA and Vulkan in the same build causes conflicts
3. Whisper alone covers most use cases

### Why Windows Only?

1. macOS doesn't support CUDA (Apple Silicon uses Metal)
2. Linux CUDA builds are possible but not tested
3. Primary development/testing is on Windows

---

## Cargo Features

```toml
[features]
default = ["vulkan"]

# CUDA build: Whisper only, NVIDIA GPU acceleration
cuda = ["dep:whisper-rs", "whisper-rs/cuda"]

# Vulkan build: Full feature set (Whisper + Parakeet + Moonshine)
vulkan = ["dep:whisper-rs", "whisper-rs/vulkan", "dep:transcribe-rs", ...]
```

---

## Fork Patches (Required)

The CUDA build requires patched versions of `whisper-rs` and `whisper-rs-sys`:

```toml
# src-tauri/Cargo.toml
[patch.crates-io]
whisper-rs-sys = { path = "C:/Code/experiments/whisper-rs-test/sys" }
whisper-rs = { path = "C:/Code/experiments/whisper-rs-test" }
```

### What the fork fixes:

1. **`sys/build.rs`**: Added `.layout_tests(false)` to bindgen
   - Prevents struct size assertions that fail on Windows
   - Linux `_G_fpos_t` = 216 bytes, Windows = 208 bytes

2. **`src/standalone.rs`**: Fixed removed API functions
   - `ggml_cpu_has_blas()` and `ggml_cpu_has_cuda()` removed in whisper.cpp v1.8.2
   - Now uses `cfg!(feature = ...)` compile-time detection

3. **`src/whisper_params.rs`**: Fixed renamed field
   - `suppress_non_speech_tokens` renamed to `suppress_nst`

4. **Windows-only error handling**: If bindgen fails on Windows, build fails immediately
   - No silent fallback to Linux bindings

5. **GGML Library Refactor (v1.8.2+)**: Updated `sys/build.rs` to link against split libraries:
   - Added `ggml-base`, `ggml-cpu`, and `ggml-cuda` (when enabled)
   - Resolves "101 unresolved externals" errors caused by the GGML refactor

---

## Why CUDA 12.4 (NOT 13.x)?

CUDA 13.x requires C++17, which causes problems:

```
fatal error C1189: CUB requires at least C++17
```

The `whisper-rs-sys 0.11.x` build script cannot pass `--std=c++17` to nvcc properly.
CUDA 12.4 doesn't have this requirement, so it "just works".

---

## Troubleshooting

### "stdbool.h not found"

Bindgen can't find Clang's builtin headers. The build script sets up paths automatically, but if it fails:

```powershell
# Check clang's resource directory
clang.exe --print-resource-dir
# Should be something like: C:\Program Files\LLVM\lib\clang\21

# Verify stdbool.h exists
Test-Path "C:\Program Files\LLVM\lib\clang\21\include\stdbool.h"
```

### "Unable to generate bindings"

If you see this warning, bindgen failed silently. Check:
1. `LIBCLANG_PATH` is set correctly
2. Visual Studio C++ workload is installed
3. Run from VS Developer Command Prompt or use `build-cuda.ps1`

### "attempt to compute 208_usize - 216_usize overflow"

You're using Linux bindings on Windows. Make sure:
1. `WHISPER_DONT_GENERATE_BINDINGS` is NOT set
2. Using the patched whisper-rs-sys fork
3. Run `.\build-cuda.ps1 -Clean -Build` to force regeneration

### CUDA version mismatch

If nvcc shows wrong version:
```powershell
# Check which nvcc is in PATH
where.exe nvcc
nvcc --version

# Force CUDA 12.4
$env:CUDA_PATH = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4"
$env:PATH = "$env:CUDA_PATH\bin;" + $env:PATH
```

---

## Files Modified from Main

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | CUDA feature, patch.crates-io for whisper-rs |
| `CUDA.md` | This documentation (doesn't exist on main) |
| `build-cuda.ps1` | Build script for CUDA (doesn't exist on main) |
| `problem.md` | Debugging notes (can be removed) |

---

## System Requirements

| Component | Requirement |
|-----------|-------------|
| OS | Windows 10/11 x64 |
| GPU | NVIDIA with CUDA Compute Capability 7.5+ |
| CUDA Toolkit | v12.4 (v13.x NOT supported) |
| Visual Studio | 2022 with C++ workload |
| LLVM/Clang | 18.x - 21.x |
| Ninja | Latest |
| Rust | Latest stable |

---

## Build Output

Successful build produces:
- A bundled executable (if using `-Full`) in `src-tauri/target/release/bundle/`
- A raw binary in `src-tauri/target/release/aivorelay.exe`

**Note:** If you run the raw binary directly without bundling assets, you might see a "connection refused" error. Use `.\build-cuda.ps1 -Full` for a portable version.

This executable includes Whisper with CUDA acceleration. On first run, it will download the Whisper model and use your NVIDIA GPU for transcription.
