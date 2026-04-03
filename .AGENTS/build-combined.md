Branch tags: #branch/codex-combined

# Combined Build Rules

Read this file only when the task needs combined build, toolchain, sidecar packaging, bindings, or verification rules.

## Build Intent

- Ship one combined Windows distribution folder with the standard app plus sidecar executables.
- Keep one shared app data/profile location across executable variants.
- Prefer release-build verification for restart and handoff behavior.

## Main Local Build Paths

Standard combined local build:

```powershell
pwsh -NoProfile -File .\build-local.ps1
```

Combined local build including the CUDA sidecar:

```powershell
pwsh -NoProfile -File .\build-local.ps1 -Cuda
```

Debug packaging check:

```powershell
pwsh -NoProfile -File .\build-local.ps1 -Debug -Cuda
```

## What The Local Build Does

- Imports the Visual Studio build environment.
- Configures bindgen include paths and `LIBCLANG_PATH`.
- Ensures `vulkan-1.dll` is present for Windows packaging.
- Uses a short `CARGO_TARGET_DIR`.
- Always prepares the AVX2 sidecar before the main build.
- Prepares the CUDA sidecar when `-Cuda` or `AIVORELAY_BUILD_CUDA=1` is used.
- Preserves output copies in `.AGENTS/.UNTRACKED/build-artifacts/<profile>` when local build cache cleanup is enabled.

## Build Modes And Scripts

- `build-local.ps1` is the main local combined build entrypoint.
- Release mode runs `bun run build:unsigned`.
- Debug mode runs `bun run tauri build --debug --no-sign`.
- `build-unsigned.js` injects the branch-specific `externalBin` list for packaging.
- `scripts/prepare-avx2-sidecar.js` builds `aivorelay-avx2.exe`.
- `scripts/prepare-cuda-sidecar.js` builds `aivorelay-cuda.exe`.

## Required Local Tools

- Visual Studio 2022 C++ build tools
- LLVM/clang in `C:\Program Files\LLVM\bin`
- Rust
- `bun`
- Vulkan SDK or a working system `vulkan-1.dll`
- CUDA Toolkit when building the CUDA sidecar

## Process Lock Rule

Before running `cargo`, `tauri`, `rustc`, or `bun` build work, check:

```powershell
Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" } | Select-Object Name, Id
```

If any such process is already running, do not start another conflicting build command.

## Important Combined Build Notes

- `src-tauri/Cargo.toml` is a workspace that includes `sidecars/aivorelay-avx2` and `sidecars/aivorelay-cuda`.
- `src-tauri/tauri.conf.json` keeps the AVX2 sidecar in `externalBin`; the CUDA sidecar is added at build time when requested.
- `src-tauri/.cargo/config.toml` uses a short target dir for branch-local Windows builds.
- `-Avx2` is for AVX2-focused local build cases; the usual combined packaging path is standard build plus sidecars.
- Combined restart/handoff behavior is better validated on packaged builds than on dev-server flows.

## Backend Tests

Use the checked-in harness:

```powershell
pwsh -NoProfile -File .\test-local.ps1
```

For documented subsets and current test areas, read `TESTING.md` in this worktree.

## Bindings

`src/bindings.ts` rules:

- Bindings are generated when the debug app actually runs, not at compile time.
- CI compiles but does not run the app, so CI cannot generate bindings.
- After changing any `#[tauri::command]` in Rust, ask the user to run the appropriate combined dev flow to regenerate bindings, unless the user explicitly asks the agent to do it.
- Only commit an updated `src/bindings.ts` when the user explicitly asks for that commit.

## Companion Doc

- For combined branch-local runtime and file-diff context, read [[.AGENTS/combined-branch-notes|combined-branch-notes.md]].
