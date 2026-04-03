# CUDA Build Rules

Read this file only when the task needs CUDA build, toolchain, bindings, or verification rules.

## Main Build Path

Use the branch-local helper script:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoBuild
```

Dev run:

```powershell
pwsh -NoProfile -File .\build-cuda.ps1 -DoDev
```

Default behavior with no switch is the same as `-DoBuild`.

## Required Local Tools

- Visual Studio 2022 C++ build tools
- LLVM in `C:\Program Files\LLVM\bin`
- CUDA Toolkit 12.4
- `bun`
- `ninja`

## Required Local Dependency Forks

Default dependency root:

- `C:\Code\AIVORelay-deps\AIVORelay-dep-transcribe-rs`
- `C:\Code\AIVORelay-deps\AIVORelay-dep-whisper-rs`

`AIVORelay-dep-whisper-rs\sys\whisper.cpp` must be checked out with submodules.

## Process Lock Rule

Before running `cargo`, `tauri`, `rustc`, or `bun` build work, check:

```powershell
Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" } | Select-Object Name, Id
```

If any such process is already running, do not start another conflicting build command.

## CUDA Branch Build Notes

- This branch uses local dependency forks and rewrites Cargo patch paths during `build-cuda.ps1`.
- `.cargo/config.toml` uses a short target dir: `C:/aivorelay-cuda`
- The branch uses `lld-link`.
- Local dev mode intentionally uses `tauri dev --release`.
- Local build path remains `--no-bundle`.

## Bindings

`src/bindings.ts` rules:

- Bindings are generated when the debug app actually runs, not at compile time.
- CI compiles but does not run the app, so CI cannot generate bindings.
- After changing any `#[tauri::command]` in Rust, ask the user to run the CUDA dev flow to regenerate bindings, unless the user explicitly asks the agent to do it.
- Only commit an updated `src/bindings.ts` when the user explicitly asks for that commit.

## Companion Docs

- For branch-local dependency and file-diff context, read [[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]].
- For the human-readable branch overview, read [[CUDA]].
