Branch tags: #branch/release-microsoft-store

# Build Store

Read this file only when the task needs build, toolchain, bindings, or verification rules for the Microsoft Store Edition.

Ask the user before running any build or test. Read-only inspection and `cargo metadata` lockfile validation are not full builds.

## Environment

- Windows 11
- PowerShell (`pwsh`) host
- Visual Studio 2022 build tools are required for Rust/Cargo work and are not in PATH by default
- Native Windows tools are available on PATH, including `rg`, `sg`, and `sd`

## Visual Studio Environment Setup

Run this once per conversation, only when Rust/Cargo tooling is actually needed:

```powershell
$vsPath = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath; cmd /c "`"$vsPath\Common7\Tools\VsDevCmd.bat`" -arch=x64 && set" | Where-Object { $_ -match '^(.+?)=(.*)$' } | ForEach-Object { Set-Item "Env:$($Matches[1])" $Matches[2] }
```

## Concurrent Build Process Rules

Before any build-related Rust tooling, check for active processes:

```powershell
Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" } | Select-Object Name, Id
```

Rules:

- If any `cargo|tauri|rustc|bun` process is already running, do not run `cargo check`, `cargo clippy`, or `cargo fmt`.
- Wait for background dev/build processes to finish before starting Rust tooling.
- Frontend-only verification is safe anytime when it does not conflict with active work.

## Safe Frontend Commands

- `bun x tsc --noEmit`
- `bun run lint`
- `bun run check:translations`

Do not run formatters in write mode.

## Rust Verification And Tests

Rust verification is allowed only when no conflicting `cargo|tauri|rustc|bun` process is already running.

Typical commands:

- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo clippy --manifest-path src-tauri/Cargo.toml`
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- `pwsh -NoProfile -File .\test-local.ps1`

Use the checked-in test harness for backend tests rather than plain `cargo test`.

## Store-Specific Build Notes

- Store releases currently target Windows x64 only.
- Keep the Store branch on its AVX2 distribution baseline and keep AVX512/AMX disabled.
- Do not reintroduce self-updater build assumptions into Store packaging.
- The Store workflow must audit x64 ZIP/MSI contents for transcribe.cpp, ggml, VC++ runtime, and OpenMP DLLs.
- ARM64 blocks in shared build/test workflows are not evidence of ARM64 Store release support.
- If a task is not branch-specific program behavior, read `AGENTS.md` on `main` first.

## Cargo.lock

- Do not copy `src-tauri/Cargo.lock` from `main`.
- After manifest changes, regenerate with `cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1`.
- Verify the result with `cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 --locked`.
- Keep reviewed Store-only dependency drift, especially updater changes, unless the user asks to change it.

## TypeScript Bindings

`src/bindings.ts` rules:

- Bindings are generated when the debug app actually runs, not at compile time.
- CI compiles the app but does not run it, so CI cannot generate bindings.
- The file must stay in git so CI has it during build.
- After changing any `#[tauri::command]` in Rust, ask the user to run `bun tauri dev` to regenerate `src/bindings.ts`, unless the user explicitly asks the agent to do it.
- Only commit an updated `src/bindings.ts` when the user explicitly asks for that commit.
