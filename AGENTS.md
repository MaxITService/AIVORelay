# Fork Agents Guide

> **Agent rule:** all debugging/build verification is done by the user (do not run automated tests/builds unless explicitly requested).
> This file provides guidance for AI code agents working with this fork.
> CODE ONLY WHEN APPROVED BY USER. Otherwise, only your thoughts in chat are needed.
> If you are not very sure that change will fix it, consult user first, user may want to revert unsuccessful fix, so user needs to commit and stuff.
> Never Commit unless received clear instruction to do so!
> Start from writing instructions about building rules only in chat to user. Write them to user!!!!

## Environment:

Windows 11; PowerShell (pwsh) host.
Harness: use PowerShell with -NoProfile only: avoid profile interference.

**CRITICAL: Environment Setup if build is needed**. This project requires Visual Studio 2022 build tools which are NOT in the path by default.

**IF ASKED TO RUN: Run Get-Dev ONCE per conversation** (not with every command). Run it as a standalone command first, then run cargo commands separately:

```powershell
# Step 1: Run this ONCE at the start of conversation (standalone command)
$vsPath = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath; cmd /c "`"$vsPath\Common7\Tools\VsDevCmd.bat`" -arch=x64 && set" | Where-Object { $_ -match '^(.+?)=(.*)$' } | ForEach-Object { Set-Item "Env:$($Matches[1])" $Matches[2] }
```

After running this once, cargo/rustc commands work for the rest of the conversation without needing to re-run it.

**Note**: Uses `vswhere.exe` to auto-detect any VS 2022 installation (Community, Professional, Enterprise, or BuildTools). The command imports VS environment variables into the current PowerShell session.

**CRITICAL: Concurrent Cargo Processes & Tooling**.

1. **Check locks FIRST**: Run `Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" } | Select-Object Name, Id`
2. **The "No-Go" Rule**: If ANY process is found, **DO NOT run `cargo check`, `cargo clippy`, or `cargo fmt`** (avoids file locks/rebuilds).
3. **Safe anytime**: frontend tools like `tsc`, `eslint`, and `prettier`.
4. **Wait**: Use `command_status` for background commands; wait for `Status: DONE`.
5. **Get-Dev Once**: Run environment setup ($vsPath) ONCE per conversation, not inline.
6. **Output Markers**: Wrap long commands: `Write-Host "--- START TASK ---"; <cmd>; Write-Host "--- END TASK ---"`

**Key Tools:** Use frontend tools (`bun x tsc --noEmit`, `bun run lint`, `bun run format`) anytime. Use Rust tools (`cargo fmt`, `cargo clippy`, `cargo check`) ONLY if NO dev processes are running.

**ast-grep (sg) and rg and also sd INSTALLED on Windows and on PATH, installed via Winget - their Windows versions!**
No need to use WSL for them: their Windows versions are installed: callable directly from PowerShell. Use the best tool, where sane, where the best tool wins, probably you also have good tools inside your harness.

## ⚠️ Fork Info
This is a **fork** of [cjpais/Handy](https://github.com/cjpais/Handy). This repo (AivoRelay) adds Windows-specific features.

## Active Branches

Only interact with these branches (ignore upstream and others):
- `main`
- `Microsoft-store`
- `cuda-integration`

(When user says "all branches", they mean ONLY these three).



## Fork Documentation

- **[`code-notes.md`](code-notes.md)**: **Complete list of fork-specific files and changes** — read this to understand what differs from upstream
- **[`AGENTS.md`](AGENTS.md)**: Original development commands and architecture (applies to both upstream and fork)
- **[`README.md`](README.md)**: Fork features overview
- **[`fork-merge-guide.md`](fork-merge-guide.md)**: Upstream tracking + merge/conflict-resolution notes (only needed when syncing from upstream)

When adding new features, please prefer adding them in new files instead of editing originals unless these are already fork-specific files.


## Guidelines for Agents

### TypeScript Bindings (`src/bindings.ts`)

- Bindings are generated when the **debug app actually runs** (not at compile time)
- CI only compiles, it never runs the app — so CI cannot generate bindings
- The file must be in git so CI has it available during build
- After modifying any `#[tauri::command]` in Rust, run `bun tauri dev` to regenerate, then commit the updated file

### When Modifying Fork Features

1. Check [`code-notes.md`](code-notes.md) to understand which files are fork-specific
2. Fork features are mostly Windows-only — use `#[cfg(target_os = "windows")]` guards
3. Settings are in `src-tauri/src/settings.rs` (look for `remote_stt`, `ai_replace_*`, `connector_*` fields)

### Adding New Fork Features

1. Add new files when possible (cleaner separation from upstream) ! So original code "is left alone" and can be merged easily, but we have something like copy, which is fully custom: less code to merge.
2. Document in `code-notes.md`
3. Add translations in `src/i18n/locales/en/translation.json`
4. Consider platform guards if Windows-specific

## Version Bump Checklist

To release: 
1. Update `"version": "x.y.z"` in `package.json` and `src-tauri/tauri.conf.json`.
2. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
3. Run `cargo check --manifest-path src-tauri/Cargo.toml` to sync `Cargo.lock`.
4. Commit (`chore: bump version to x.y.z`), tag (`vx.y.z`), and push `main` & tag.
