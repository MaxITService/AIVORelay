# Build Environment

Read this file only when the task needs build, toolchain, bindings, or verification rules.

## Environment

- Windows 11
- PowerShell (`pwsh`) host
- Visual Studio 2022 build tools are required for Rust/Cargo build work and are not in PATH by default
- Native Windows tools are available on PATH, including `rg`, `sg`, and `sd`

## Visual Studio Environment Setup

Run this once per conversation, only when Rust/Cargo tooling is actually needed:

```powershell
$vsPath = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath; cmd /c "`"$vsPath\Common7\Tools\VsDevCmd.bat`" -arch=x64 && set" | Where-Object { $_ -match '^(.+?)=(.*)$' } | ForEach-Object { Set-Item "Env:$($Matches[1])" $Matches[2] }
```

After this runs once, `cargo` and `rustc` commands work for the rest of the conversation without rerunning it.

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

These are normally safe when only frontend verification is needed:

- `bun x tsc --noEmit`
- `bun run lint`
- `bun run format:frontend`
- `bun run check:translations`

## Rust Verification

Rust verification is allowed only when no conflicting `cargo|tauri|rustc|bun` process is already running.

Typical commands:

- `cargo check`
- `cargo clippy`
- `cargo fmt`

## Output Markers

Wrap long-running commands with clear markers:

```powershell
Write-Host "--- START TASK ---"
<command>
Write-Host "--- END TASK ---"
```

## TypeScript Bindings

`src/bindings.ts` rules:

- Bindings are generated when the debug app actually runs, not at compile time.
- CI compiles the app but does not run it, so CI cannot generate bindings.
- The file must stay in git so CI has it during build.
- After changing any `#[tauri::command]` in Rust, ask the user to run `bun tauri dev` to regenerate `src/bindings.ts`, unless the user explicitly asks the agent to do it.
- Only commit an updated `src/bindings.ts` when the user explicitly asks for that commit.
