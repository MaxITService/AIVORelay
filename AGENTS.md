# Fork Agents Guide

> **CRITICAL: WE ARE ON THE `Microsoft-store` BRANCH.**
> This branch is specifically for the Microsoft Store release.
> **AGENT RULE:** Always refer to this version as the **Microsoft Store Edition**.
> All updates must be compliant with Microsoft Store policies (e.g., no self-updating, sandboxed file access in mind (MSIX packaged, this will be handled atomatically later)). Warn the user in case something is not compatible with the Microsoft Store.
> **Agent rule:** the user still owns final debugging/build verification, but agents may run `cargo check`, `cargo clippy`, `cargo fmt`, and similar non-conflicting verification commands when no conflicting `cargo`/`tauri`/`rustc`/`bun` process is already running.
> This file provides guidance for AI code agents working with this fork.
> If you are not very sure that change will fix it, consult user first, user may want to revert unsuccessful fix, so user needs to commit and stuff.
> Use git commit with proper message, after you feel that application is at the completed step, ready for testing. Amend in case that user asked you to fix something related to previous commit, so we don't commit anything improper. Do not push untill asked
> Start from writing instructions about building rules only in chat to user. Write them to user!!!!

## Environment:

Windows 11; PowerShell (pwsh) host.
Harness: use PowerShell with -NoProfile only: avoid profile interference.
This is a brilliant codebase that balances security, functionality, and simplicity. The application itself is a functional and beautiful piece of art. Treat this app with care and respect.


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
3. **Safe anytime**: frontend-only tools like `bun x tsc --noEmit`, `bun run lint`, `bun run format:frontend`, and `bun run check:translations`.
4. **Wait**: If a background dev/build command is already running, do not start Cargo tools until it has clearly finished.
5. **Get-Dev Once**: Run environment setup ($vsPath) ONCE per conversation, not inline.
6. **Output Markers**: Wrap long commands: `Write-Host "--- START TASK ---"; <cmd>; Write-Host "--- END TASK ---"`
7. **To check processes**: `Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" } | Select-Object Name, Id`

**Key Tools:** Use frontend-only tools (`bun x tsc --noEmit`, `bun run lint`, `bun run format:frontend`, `bun run check:translations`) anytime. Use Rust tools (`cargo fmt`, `cargo clippy`, `cargo check`) ONLY if NO dev processes are running.

**ast-grep (sg) and rg and also sd INSTALLED on Windows and on PATH, installed via Winget - their Windows versions!**
No need to use WSL for them: their Windows versions are installed: callable directly from PowerShell. Use the best tool, where sane, where the best tool wins, probably you also have good tools inside your harness.

## ⚠️ Fork Info
This is a **fork** of [cjpais/Handy](https://github.com/cjpais/Handy). This repo (AivoRelay) adds Windows-specific features.

## Active Branches

Only interact with these branches:
- `main`
- `Microsoft-store`
- `cuda-integration`

(When user says "all branches", they currently mean `main`, `Microsoft-store`, and `cuda-integration`.)
For work on non-`main` branches, use `main` as the only sync source.

## Fork Documentation - Read file(s) that is related to current task ONLY.

Docs use Obsidian-style links like `[[path|label]]`.

- [[.AGENTS/code-notes|code-notes.md]]: complete list of fork-specific files and changes
- [[AGENTS]]: entry file
- [[README]]: fork features overview
- [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]]: `upstream -> main` intake playbook
- [[.AGENTS/main-to-microsoft-store-propagation-playbook|main-to-microsoft-store-propagation-playbook.md]]: `main -> Microsoft-store` propagation playbook
- [[.AGENTS/main-to-cuda-propagation-playbook|main-to-cuda-propagation-playbook.md]]: `main -> cuda-integration` propagation playbook
- [[.AGENTS/upstream-OrBranches-intake-playbook|upstream-OrBranches-intake-playbook.md]]: compatibility index pointing to the branch-specific playbooks above
- [[.AGENTS/upstream-sync-log|upstream-sync-log.md]]: rolling log of last synced upstream commits (max 10)
- [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]: rolling log of last propagated `main` commits into release branches (max 10)
- [[.AGENTS/branching-status|branching-status.md]]: branch sync/cherry-pick status


When adding new features, please prefer adding them in new files instead of editing originals unless these are already fork-specific files.
### Agent Temp Files And Doc Updates

- Temporary agent-only files may be placed in `.AGENTS/.UNTRACKED/`.
- If the user asks to update documentation, keep new text maximally concise.


## Guidelines for Agents

### TypeScript Bindings (`src/bindings.ts`)

- Bindings are generated when the **debug app actually runs** (not at compile time)
- CI only compiles, it never runs the app — so CI cannot generate bindings
- The file must be in git so CI has it available during build
- After modifying any `#[tauri::command]` in Rust, ask the user to run `bun tauri dev` to regenerate `src/bindings.ts`, or run it yourself only if the user explicitly requests it
- Only commit the updated `src/bindings.ts` when the user explicitly instructs you to commit

### When Modifying Fork Features

1. Check [[.AGENTS/code-notes|code-notes.md]] to understand which files are fork-specific
2. Fork features are mostly Windows-only — use `#[cfg(target_os = "windows")]` guards
3. Settings are in `src-tauri/src/settings.rs` (look for `remote_stt`, `ai_replace_*`, `connector_*` fields)

### Adding New Fork Features

1. Add new files when possible (cleaner separation from original files) ! So original code "is left alone" and can be merged easily, but we have something like copy, which is fully custom: less code to merge.
2. Document in [[.AGENTS/code-notes|code-notes.md]]
3. Add translations in `src/i18n/locales/en/translation.json`
4. Consider platform guards if Windows-specific


## Version Bump Checklist

When asked to bump version or prepare a release, read [[.AGENTS/Release|Release.md]].
