# CUDA Build Notes

This branch uses a local-only CUDA setup. Nothing here requires global environment changes or registry edits.

## What is patched locally

- `src-tauri/Cargo.toml` redirects `transcribe-rs`, `whisper-rs`, and `whisper-rs-sys` to local forks in `C:\Code\AIVORelay-deps`.
- `C:\Code\AIVORelay-deps\AIVORelay-dep-transcribe-rs` is the local `transcribe-rs` dependency fork used by AIVORelay.
- `C:\Code\AIVORelay-deps\AIVORelay-dep-whisper-rs` contains the Windows CUDA bindgen/API fixes needed by AIVORelay.
- `build-cuda.ps1` rewrites the local `[patch.crates-io]` paths before building, so the same branch can work with a different dependency root.
- `.cargo/config.toml` uses a short target directory: `C:/aivorelay-cuda`

## Planned repository names for CI

The CUDA GitHub Actions release workflow expects these dependency repositories:

- `MaxITService/AIVORelay-dep-transcribe-rs`
- `MaxITService/AIVORelay-dep-whisper-rs`

Locally, `build-cuda.ps1` still works from the filesystem copies in `C:\Code\AIVORelay-deps`.

## Required local tools

- Visual Studio 2022 C++ build tools
- LLVM with `clang.exe` and `libclang.dll` in `C:\Program Files\LLVM\bin`
- CUDA Toolkit 12.4 in `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4`
- `bun`
- `ninja`

## Dependency sources

Local dependency directories:

- `C:\Code\AIVORelay-deps\AIVORelay-dep-transcribe-rs`
- `C:\Code\AIVORelay-deps\AIVORelay-dep-whisper-rs`
  - `sys/whisper.cpp` inside this repo is a **git submodule** — must be checked out with `--recurse-submodules`
  - Required header: `C:\Code\AIVORelay-deps\AIVORelay-dep-whisper-rs\sys\whisper.cpp\include\whisper.h`

Public GitHub repositories used by the CUDA release workflow:

- `https://github.com/MaxITService/AIVORelay-dep-transcribe-rs`
- `https://github.com/MaxITService/AIVORelay-dep-whisper-rs`

## Build modes

| Mode | Command | Purpose | Output |
| --- | --- | --- | --- |
| Local release build | `pwsh -NoProfile -File .\build-cuda.ps1 -DoBuild` | Main local CUDA build path | `C:\aivorelay-cuda\release\` |
| Local dev run | `pwsh -NoProfile -File .\build-cuda.ps1 -DoDev` | Launch `tauri dev --release` with CUDA env | live dev process + logs |
| Default behavior | `pwsh -NoProfile -File .\build-cuda.ps1` | Same as `-DoBuild` | `C:\aivorelay-cuda\release\` |
| GitHub Actions CUDA release | `.github/workflows/cuda-release.yml` | Draft release + portable zip | `vX.Y.Z-cuda` draft release asset |

## Build

Release build with embedded frontend assets:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoBuild
```

Dev mode is available, but release build is the recommended path:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoDev
```

If the dependencies live somewhere else on disk, pass `-DependencyRoot` explicitly:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoBuild -DependencyRoot "D:\somewhere\AIVORelay-deps"
```

If CUDA 12.4 lives somewhere else on disk, pass `-CudaPath` explicitly:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoBuild -CudaPath "D:\CUDA\v12.4"
```

`-DoDev` uses `tauri dev --release` on purpose. Debug CUDA builds on Windows currently fail in `nvcc` compiler detection with `The input line is too long`, while the release-mode dev run works around that issue.

## Output

With the short target directory, the main binary lands under:

`C:\aivorelay-cuda\release\`

The short target directory is required: Windows has a ~255-character path limit, and the CUDA/GGML CMake build tree generates deeply nested object file paths that exceed it if the target dir stays inside the source tree.

The build script uses `tauri build --no-bundle` on purpose, because that is the simplest path to a CUDA-enabled executable without dealing with MSI packaging problems first.

Generated logs next to the repo root:

- `bun-install.log`
- `tauri-build.log`
- `tauri-dev.log`

## GitHub Actions release mode

The CUDA release workflow builds the same no-bundle executable and uploads a portable zip to the draft release tag `vX.Y.Z-cuda`.

Current release behavior:

- builds from the `cuda-integration` branch
- checks out the two public dependency repos
  - `AIVORelay-dep-whisper-rs` **must** be checked out with `submodules: recursive` (whisper.cpp is a submodule inside it)
- installs LLVM, Ninja, Rust, Bun, and CUDA 12.4
- runs `build-cuda.ps1 -DoBuild`
- uploads a portable zip, not an MSI installer

## Runtime note

This branch currently builds a CUDA-enabled executable, but it does not bundle NVIDIA CUDA runtime libraries into the app package.

That means:

- local builds depend on an installed CUDA toolkit during build
- runtime still expects the required NVIDIA/CUDA libraries to be available on the target system
- the portable CUDA release is currently aimed at NVIDIA systems with matching runtime support, not as a fully self-contained universal Windows package

## Non-obvious build constraints

- **Linker:** `.cargo/config.toml` configures `lld-link` as the linker for this branch. This is  for speed
- **No MSI path:** MSI/NSIS bundling is not supported yet for the CUDA build; portable zip only.
- **CUDA version lock:** Must use CUDA **12.4**. CUDA 13.x requires C++17, which `whisper-rs-sys 0.11.x` cannot pass to `nvcc`.
