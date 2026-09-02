# Build Environment
Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

Read this file only when the task needs build, toolchain, bindings, or verification rules.

## Environment

- Windows 11
- PowerShell (`pwsh`) host
- Visual Studio 2022 build tools are required for Rust/Cargo build work and are not in PATH by default
- Native Windows tools are available on PATH, including `rg`, `sg`, and `sd`

## Visual Studio Environment Setup

Prefer the checked-in setup helper for agent-run Rust verification. It loads Visual Studio, bindgen include paths, LLVM/libclang, and a short Cargo target dir:

```powershell
. .\scripts\setup-rust-build-env.ps1
$ctx = Initialize-RustBuildEnvironment -PreferredCargoTargetDir "Q:\t\c"
try {
  cargo check --manifest-path src-tauri/Cargo.toml
} finally {
  Restore-RustBuildEnvironment $ctx
}
```

Do not run bare `cargo check` from a fresh no-profile shell. In Codex/tool shells, environment changes may not survive into the next command, so initialize and run Cargo in the same PowerShell command unless you are using a persistent terminal session.

Use the user's interactive dev functions for full dev builds instead of reproducing them manually. See [[.AGENTS/USERs_BUILD_FUNCTIONS|USERs_BUILD_FUNCTIONS.md]] for `Dev-AivoRelay` and `Fast-Dev-AivoRelay`.

## Build and Test Workflows Are Separate

- `Dev-AivoRelay` and `Fast-Dev-AivoRelay` run the interactive dev build with `CARGO_TARGET_DIR=Q:\t\d`.
- `build-local.ps1` creates an unsigned local app build and normally uses `Q:\b`.
- `test-local.ps1` runs `cargo test` in an isolated test cache and normally uses `Q:\t\c`.
- The GitHub `Build Test` and release workflows create Tauri application packages; they do not run the Rust test suite.

A native dependency failure while preparing `cargo test` in `Q:\t\c` does not by itself show that the dev build, release build, or a GitHub package build is broken. Identify which workflow and target directory failed before changing build scripts or dependencies.

Use `test-local.ps1` for backend tests. If AivoRelay is running, leave its process guard enabled: close the app before runtime tests, or use `-NoRun` when compile-only verification is sufficient. Do not use `-SkipChecks` merely to work around a running production instance.

`-Filter` accepts one or more Rust test-name filters. Pass an array with `-Exact` to compile once and run several exact tests through one test-runner process:

```powershell
$tests = @(
  "module::tests::first_test"
  "module::tests::second_test"
)
.\test-local.ps1 -LibOnly -Exact -Filter $tests
```

### Agent-shell junction caveat

`transcribe-cpp-sys` uses a short junction under `%LOCALAPPDATA%\tcs` while compiling on Windows. An agent-hosted/tool shell may be unable to create files through that junction even when the same checkout and command work in the user's interactive PowerShell. A characteristic false failure is `transcribe-cpp-sys` stopping before the test binaries run with `The directory name is invalid. (os error 267)`.

If this exact failure occurs, ask the user to run `pwsh -NoProfile -File .\test-local.ps1` from the repository root in their ordinary interactive PowerShell before diagnosing the source tree or toolchain. If that run compiles and starts the Rust tests, treat the earlier result as an agent-shell limitation. Do not change Windows policy, ACLs, security software, Cargo dependencies, the junction implementation, or production code to work around it.

## Concurrent Build Process Rules

Before any build-related Rust tooling, check for active processes:

```powershell
Get-CimInstance Win32_Process |
  Where-Object {
    $_.Name -match '^(cargo|tauri|rustc|bun|MSBuild|cmake|cl|link)(\.exe)?$' -and
    -not ($_.Name -match '^MSBuild(\.exe)?$' -and $_.CommandLine -match '/nodemode:1')
  } |
  Select-Object ProcessId, Name, CommandLine
```

Rules:

- If any `cargo|tauri|rustc|bun|MSBuild|cmake|cl|link` process is already running, do not run `cargo check`, `cargo clippy`, or `cargo fmt`.
- Ignore an idle MSBuild node with `/nodemode:1 /nodeReuse:true`; that is Visual Studio's reusable worker, not an active Cargo build.
- Wait for background dev/build processes to finish before starting Rust tooling.
- If active processes look stale or unrelated, stop and ask the user before killing them.
- Frontend-only verification is safe anytime when it does not conflict with active work. `bun` dev/build processes still count as conflicts for Rust verification because Tauri may be driving Cargo underneath.

## Safe Frontend Commands

These are normally safe when only frontend verification is needed:

- `bun run test:frontend`
- `bun run test:frontend:coverage`
- `bun x tsc --noEmit`
- `bun run lint`
- `bun run format:frontend`
- `bun run check:translations`

## Rust Verification

Rust verification is allowed only when no conflicting `cargo|tauri|rustc|bun|MSBuild|cmake|cl|link` process is already running.

Typical commands, wrapped in the helper above:

- `cargo check`
- `cargo clippy`
- `cargo fmt`

For formatting only, prefer `rustfmt <touched files>` when a full Cargo command would conflict with a running dev/build process.

## Known Rust Build Failure Signatures

- `stdbool.h file not found` from `whisper-rs-sys` / bindgen usually means the MSVC and Windows SDK include paths were not exported. Re-run through `scripts/setup-rust-build-env.ps1`; do not retry bare Cargo.
- `FileTracker : error FTK1011` or missing `.tlog` files under `src-tauri\target\debug\build\whisper-rs-sys...` usually means the target path is too deep, a previous native build is still running, or MSBuild scratch dirs are corrupted. Use the short `CARGO_TARGET_DIR` from the helper (`Q:\t\c`) and wait for all Cargo/Tauri/MSBuild work to finish before retrying.
- If a short target dir itself appears corrupted, ask before deleting build cache directories.
- A successful `npm run typecheck` does not prove Rust command bindings are regenerated; it only proves the current checked-in `src/bindings.ts` is type-compatible.

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
- Do not hand-edit `src/bindings.ts`.
- After changing any `#[tauri::command]` in Rust, ask the user to run `bun tauri dev` to regenerate `src/bindings.ts`, unless the user explicitly asks the agent to do it.
- If `src/bindings.ts` was already dirty before the task, call that out and avoid mixing unrelated generated churn into the change.
- Only commit an updated `src/bindings.ts` when the user explicitly asks for that commit.

## What user uses? (read only if discussing user's path to build)

User uses dev build via following functions, described in [[.AGENTS/USERs_BUILD_FUNCTIONS|USERs_BUILD_FUNCTIONS.md]].
