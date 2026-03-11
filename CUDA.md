# CUDA Build Notes

This branch uses a local-only CUDA setup. Nothing here requires global environment changes or registry edits.

## What is patched locally

- `src-tauri/Cargo.toml` redirects `transcribe-rs`, `whisper-rs`, and `whisper-rs-sys` to local forks in `C:\Code\experiments`.
- `C:\Code\experiments\transcribe-rs-test` uses `whisper-rs` with the `cuda` feature on Windows.
- `C:\Code\experiments\whisper-rs-test` contains the bindgen/API fixes needed for Windows + CUDA.
- `.cargo/config.toml` uses a short target directory: `C:/Code/build/aivorelay-cuda`

## Required local tools

- Visual Studio 2022 C++ build tools
- LLVM with `clang.exe` and `libclang.dll` in `C:\Program Files\LLVM\bin`
- CUDA Toolkit 12.4 in `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4`
- `bun`
- `ninja`

## Build

Release build with embedded frontend assets:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoBuild
```

Dev mode is available, but release build is the recommended path:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoDev
```

## Output

With the short target directory, the main binary lands under:

`C:\Code\build\aivorelay-cuda\release\`

The build script uses `tauri build --no-bundle` on purpose, because that is the simplest path to a CUDA-enabled executable without dealing with MSI packaging problems first.
