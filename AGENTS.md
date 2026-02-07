# Fork Agents Guide

> **Agent rule:** all debugging/build verification is done by the user (do not run automated tests/builds unless explicitly requested).
> This file provides guidance for AI code agents working with this fork.
> CODE ONLY WHEN APPROVED BY USER. Otherwise, only your thoughts in chat are needed.
> If you are not very sure that change will fix it, consult user first, user may want to revert unsuccessful fix, so user needs to commit and stuff.
> Never Commit!
> Start from writing instructions about building rules only in chat to user. Write them to user!!!!

## Environment:

Windows 11; PowerShell (pwsh) host.
Harness: use PowerShell with -NoProfile only: avoid profile interference.

**CRITICAL: Environment Setup**. This project requires Visual Studio 2022 build tools which are NOT in the path by default.

**Run Get-Dev ONCE per conversation** (not with every command). Run it as a standalone command first, then run cargo commands separately:

```powershell
# Step 1: Run this ONCE at the start of conversation (standalone command)
$vsPath = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -property installationPath; cmd /c "`"$vsPath\Common7\Tools\VsDevCmd.bat`" -arch=x64 && set" | Where-Object { $_ -match '^(.+?)=(.*)$' } | ForEach-Object { Set-Item "Env:$($Matches[1])" $Matches[2] }
```

After running this once, cargo/rustc commands work for the rest of the conversation without needing to re-run it.

**Note**: Uses `vswhere.exe` to auto-detect any VS 2022 installation (Community, Professional, Enterprise, or BuildTools). The command imports VS environment variables into the current PowerShell session.

**CRITICAL: Concurrent Cargo Processes & Tooling**.

1. **Check for locks FIRST**: Use this exact snippet before running any Cargo/Tauri command:
   `Get-Process | Where-Object { $_.Name -match "cargo|tauri|rustc|bun" } | Select-Object Name, Id`
2. **The "No-Go" Rule**: If ANY processes are found: **DO NOT run `cargo check`, `cargo clippy`, or `cargo fmt`**.
   - _Why?_ `check`/`clippy` fail due to Windows file locks in `target`.
   - _Why `fmt`?_ Even though it doesn't touch `target`, it modifies source files which triggers the dev-server watcher to instantly recompile/restart the app, disrupting active testing.
3. **Safe to run anytime**: `tsc` (Type check), `eslint` (Linting), and `prettier` (Formatting) are safe to run even if the dev-server is active, as they operate only on frontend assets and don't trigger Rust re-compilation.
4. **Wait for completion**: If you start a background command, you MUST use `command_status` until it returns `Status: DONE`. Do not propose new commands while one is running.
5. **Get-Dev Once Per Conversation**: Run the environment setup command ONCE at the start. Do NOT inline it with every cargo command—this causes "input line too long" errors.
6. **Use Output Markers**: For long commands, wrap them in markers:
   `Write-Host "--- START TASK ---"; cargo check --manifest-path src-tauri/Cargo.toml; Write-Host "--- END TASK ---"`
7. **Tool Discretion**: Use these tools ONLY when needed for verification. They are NOT mandatory for every small change. Do not run them blindly if you are confident in your edits.

**Key Tools:**
| Tool | command | Purpose | Safe with Dev Server? |
| :--- | :--- | :--- | :--- |
| **TSC** | `bun x tsc --noEmit` | Frontend Type Checking | ✅ YES |
| **ESLint** | `bun run lint` | Frontend Style/Logic Police | ✅ YES |
| **Prettier**| `bun run format` | Frontend Code Formatting | ✅ YES |
| **Cargo Fmt**| `cargo fmt` | Rust Code Formatting | ❌ NO (triggers re-build) |
| **Clippy** | `cargo clippy` | Rust "Smart" Linter | ❌ NO (file locks) |
| **Check** | `cargo check` | Rust Compilation Check | ❌ NO (file locks) |

**ast-grep (sg) and rg and also sd INSTALLED on Windows and on PATH, installed via Winget - their Windows versions!**
No need to use WSL for them: their Windows versions are installed: callable directly from PowerShell. Use the best tool, where sane, where the best tool wins, probably you also have good tools inside your harness.

## ⚠️ Important: This is a Fork

This repository is a **fork** of [cjpais/Handy](https://github.com/cjpais/Handy).

- **Upstream**: https://github.com/cjpais/Handy (original project)
- **This fork**: AivoRelay — Adds Windows-specific features and external service integrations

## Important: Branches:

Currently there are ONLY follwing active branches:

- `main` - main branch, contains latest stable release
- `Microsoft-store` - branch for Microsoft Store version, contains latest development version, avx512 turned off, avx2 is in use.
- `AVX2` - branch for AVX2 version, identical to main butavx512 turned off, avx2 is in use for older processors.
- `cuda-integration` - branch for CUDA integration, identical to main but with CUDA integration. In development.

IGNORE OTHER BRANCHES! IGNORE UPSTREAM BRANCHES! WHEN USER SAID "ALL BRANCHES" OR "ALL BRANCHES", THEN ONLY THESE BRANCHES: `main`, `Microsoft-store`, `AVX2`, `cuda-integration`.

### Git Workflow: Cherry-picking

When asked to "cherry-pick commit to all branches", this refers to these specific target branches:
- `main`
- `Microsoft-store`
- `AVX2`
- `cuda-integration`

**Execution steps for Agent:**
1. Record the current branch and ensure the git status is clean.
2. For each target branch:
   - Switch to the branch.
   - Verify if the commit hash already exists in that branch's history (skip if it does).
   - Run `git cherry-pick -x <hash>`.
   - If conflicts occur, **stop and report immediately** (listing the branch and conflicting files).
3. Return to the **original starting branch** at the end.
4. Provide a concise status report: `branch -> status (success/skipped/conflict)` and the latest commit info for each target branch.

## Fork Documentation

| File                                         | Description                                                                                               |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| [`code-notes.md`](code-notes.md)             | **Complete list of fork-specific files and changes** — read this to understand what differs from upstream |
| [`AGENTS.md`](AGENTS.md)                     | Original development commands and architecture (applies to both upstream and fork)                        |
| [`README.md`](README.md)                     | Fork features overview                                                                                    |
| [`fork-merge-guide.md`](fork-merge-guide.md) | Upstream tracking + merge/conflict-resolution notes (only needed when syncing from upstream)              |

When adding new features, please prefer adding them in new files instead of editing originals unless these are already fork-specific files.

## Fork-Specific Features

### 1. Remote STT API (Windows only)

- Files: `src-tauri/src/managers/remote_stt.rs`, `src/components/settings/remote-stt/`
- Uses OpenAI-compatible `/audio/transcriptions` endpoint
- API keys stored in Windows Credential Manager via `keyring` crate

**Note:** On Windows, **all LLM API keys** (Remote STT, Post-Processing, AI Replace) are stored securely in the Windows Credential Manager via `src-tauri/src/secure_keys.rs`. Existing keys from JSON settings are auto-migrated on first launch.

#### Status & Flow

| State          | Display      | Description                              |
| :------------- | :----------- | :--------------------------------------- |
| `recording`    | Red pulsing  | User is recording audio                  |
| `transcribing` | Blue pulsing | Audio sent to API, waiting for response  |
| `error`        | Red text     | API error occurred (auto-hides after 3s) |

- **Errors**: Categorized (TLS, Network, Server, Parse, etc.).
- **Logic**: Stop recording → `transcribing` state → API call → Success (paste) OR Error (show categorization + emit `remote-stt-error` event).

**Implementation files:**

- `src-tauri/src/plus_overlay_state.rs` — Error categorization and overlay control
- `src/overlay/plus_overlay_states.ts` — TypeScript types and display text
- `src/overlay/RecordingOverlay.tsx` — Overlay UI (modified to handle extended states)

### 2. AI Replace Selection (Windows only)

- Files: `src-tauri/src/actions.rs` (AiReplaceSelectionAction), `src/components/settings/ai-replace/`
- Captures selected text, sends to LLM with voice instruction, replaces selection
- Selection capture uses Windows-specific APIs in `src-tauri/src/input.rs`
- **Optional separate LLM configuration**: Configure a different provider/model specifically for AI Replace in "LLM API Relay" settings. Falls back to post-processing defaults.

### 3. Connector (Send Transcription to Extension)

- Primary implementation: `src-tauri/src/managers/connector.rs` (local HTTP server), `src-tauri/src/actions.rs` (Send\*ToExtension actions), `src/components/settings/browser-connector/`
- Extension polls AivoRelay's local server (default `http://127.0.0.1:38243`):
  - `GET /messages?since=<cursor>` → queued messages + next `cursor`
  - `GET /blob/{attId}` → attachment bytes (short-lived)
  - `POST /messages` → extension acks (e.g. `keepalive_ack`, `password_ack`)
- **Auth required:** `Authorization: Bearer <connector_password>` (see `src-tauri/src/settings.rs`; password may rotate via `passwordUpdate` handshake)

### 4. Send Transcription + Screenshot to Extension (Windows only)

- Files: `src-tauri/src/actions.rs` (SendScreenshotToExtensionAction), `src-tauri/src/managers/connector.rs` (blob serving), `src-tauri/src/region_capture.rs`
- **Default**: Uses **Native Region Capture** (Windows only) - draws a selection overlay directly on screen.
- **Alternative**: Can use external tools like ShareX (configure in settings).
- Watches screenshot folder for new images using `notify` crate (if using external tool)
- Sends bundle message with image attachment and voice instruction to extension
- Configurable: capture command, folder path, timeout, "require recent" filter, "allow without voice"
- Settings: `screenshot_capture_method`, `screenshot_capture_command`, `screenshot_folder`, etc.

### 5. Transcription Profiles

- Files: `src-tauri/src/settings.rs` (TranscriptionProfile struct), `src-tauri/src/shortcut.rs` (profile commands), `src/components/settings/TranscriptionProfiles.tsx`
- Create custom shortcuts with specific language, translation, and system prompt settings
- Each profile creates a dynamic shortcut binding (e.g., `transcribe_profile_1234567890`)
- Profile's `system_prompt` overrides global per-model prompt when set
- **System Prompt Limits**: Enforced based on active STT model (Whisper: 896 chars, Deepgram: 2000 chars)
- Character limit logic shared between `TranscriptionSystemPrompt.tsx` (frontend) and `managers/remote_stt.rs` (backend)

### 6. Voice Command Center (Windows only)

- Files: `src-tauri/src/commands/voice_command.rs`, `src/components/settings/voice-commands/`
- Execute PowerShell scripts via voice commands (e.g., "lock computer", "open notepad")
- **Safe Execution**: Always shows a confirmation overlay before running any command
- **LLM Fallback**: If no predefined command matches, uses an LLM to generate a PowerShell one-liner on the fly
- **Modes**: Predefined (fast, offline) vs. LLM (flexible, requires API key)

### 7. Transcribe Audio File

- Files: `src-tauri/src/commands/file_transcription.rs`, `src/components/settings/transcribe-file/`
- Drag-and-drop interface for transcribing existing audio files
- Supports `wav`, `mp3`, `m4a`, `ogg`, `flac`, `webm`
- **Output Formats**: Plain Text, SRT (Subtitles), VTT (Web Video Text Tracks)
- **Timestamping**: Accurate timestamps are available when using **Local Whisper** models
- **Remote STT**: Currently produces text-only output (no segment timestamps)

### 8. Text Replacement

- Files: `src-tauri/src/settings.rs` (TextReplacement struct), `src/components/settings/text-replacement/`
- Automatic find-and-replace for transcription output
- **Escape Sequences**: Supports `\n` (LF), `\r\n` (CRLF), `\r` (CR), `\t` (tab), `\\` (backslash)
- **Options per rule**:
  - `case_sensitive` — case-sensitive or case-insensitive matching (default: true)
  - `is_regex` — treat "from" pattern as regular expression (supports `$1`, `$2` capture groups)
- Applied after all processing (Chinese conversion, LLM post-processing)
- Each rule can be individually enabled/disabled
- **Use Cases**: Fix commonly misheard words, apply formatting, normalize punctuation, remove repeated words

## Guidelines for Agents

### TypeScript Bindings (`src/bindings.ts`)

**IMPORTANT:** The file `src/bindings.ts` is auto-generated but **must be tracked in git**. Do NOT add it to `.gitignore`.

- Bindings are generated when the **debug app actually runs** (not at compile time)
- CI only compiles, it never runs the app — so CI cannot generate bindings
- The file must be in git so CI has it available during build
- After modifying any `#[tauri::command]` in Rust, run `bun tauri dev` to regenerate, then commit the updated file

### When Modifying Fork Features

1. Check [`code-notes.md`](code-notes.md) to understand which files are fork-specific
2. Fork features are mostly Windows-only — use `#[cfg(target_os = "windows")]` guards
3. Settings are in `src-tauri/src/settings.rs` (look for `remote_stt`, `ai_replace_*`, `connector_*` fields)

### Adding New Fork Features

1. Add new files when possible (cleaner separation from upstream) ! So original code "is left alone" and can be merged easily, but we have like copy, which is fully custom: less code to merge.
2. Document in `code-notes.md`
3. Add translations in `src/i18n/locales/en/translation.json`
4. Consider platform guards if Windows-specific

## Upstream Sync / Merging

See [`fork-merge-guide.md`](fork-merge-guide.md) for upstream tracking and the merge/conflict-resolution checklist.

## Local Builds (Unsigned)

### Quick Build (recommended)

```bash
bun run build:unsigned
```

This is the fastest way to build locally. Produces unsigned MSI without auto-update functionality.

**Prerequisites:** VS environment configured, `vulkan-1.dll` in `src-tauri/` (copy from `C:\Windows\System32\` if missing).

**Output:** `src-tauri\target\release\bundle\msi\AivoRelay_x.x.x_x64_en-US.msi`

### Full Build Script (first-time setup)

If environment is not configured or you're unsure:

```powershell
.\build-local.ps1           # Release build with all checks
.\build-local.ps1 -Debug    # Debug build
.\build-local.ps1 -SkipChecks  # Skip process/environment checks
```

**What it does:**
1. Checks for running cargo/tauri processes
2. Sets up Visual Studio environment
3. Copies `vulkan-1.dll` to src-tauri (if missing)
4. Checks for required tools (bun, cargo)
5. Runs `bun install`
6. Runs `bun run build:unsigned`

**Note:** Unsigned builds do NOT have auto-update functionality.
