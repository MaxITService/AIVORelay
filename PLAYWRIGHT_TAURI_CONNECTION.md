# Playwright + Tauri Connection

This document describes the working Playwright connection flow for AivoRelay on Windows.

## What Works Now

- The same visible Tauri dev window can be controlled through Playwright.
- The app must be launched with `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT`.
- The main Tauri window is created in `src-tauri/src/lib.rs`, not from `tauri.conf.json`.
- On Windows, Playwright mode uses a separate WebView2 user-data directory so the visible UI stays stable.

## Standard Launch Paths

### User path

If you want the normal user-facing dev flow with Playwright enabled:

```powershell
Fast-Dev-AivoRelay -EnablePlaywright
```

or:

```powershell
Dev-AivoRelay -EnablePlaywright
```

These functions live in the user's PowerShell profile and are documented in [[.AGENTS/USERs_BUILD_FUNCTIONS|USERs_BUILD_FUNCTIONS.md]].

### Checked-in repo path

If you want a checked-in launcher that does not depend on the user's PowerShell profile:

```powershell
pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1
```

You can also choose another port:

```powershell
pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1 -PlaywrightPort 9334
```

This path is the simplest one for agents because it is local to the repo and can be invoked directly.

## What The Launcher Does

`scripts/start-playwright-tauri-dev.ps1`:

- loads the Rust/MSVC environment through `scripts/setup-rust-build-env.ps1`
- sets `CARGO_TARGET_DIR` to a short Windows-safe path
- ensures `src-tauri/vulkan-1.dll` exists
- sets `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT`
- starts `bun x tauri dev`

## Verify That CDP Is Open

After the app starts, this should return JSON:

```powershell
Invoke-WebRequest -UseBasicParsing http://127.0.0.1:9333/json/version | Select-Object -ExpandProperty Content
```

If that endpoint is not available, Playwright will not be able to attach.

## Connect From Playwright

Minimal Playwright attach example:

```ts
import { chromium } from "playwright";

const browser = await chromium.connectOverCDP("http://127.0.0.1:9333");
const context = browser.contexts()[0];
const page = context.pages()[0];

console.log(await page.title());
```

If you want to inspect the available CDP targets first:

```powershell
Invoke-WebRequest -UseBasicParsing http://127.0.0.1:9333/json/list | Select-Object -ExpandProperty Content
```

## Important Implementation Notes

- The app is single-instance through `tauri-plugin-single-instance`.
- Do not plan around "one visible instance for the user and another for Playwright".
- The correct model is one shared instance with CDP enabled.
- `src-tauri/src/lib.rs` is the source of truth for the main window and WebView2 browser args.

## Troubleshooting

### CDP port is missing

Cause:

- the app was launched without `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT`

Fix:

- relaunch with `Fast-Dev-AivoRelay -EnablePlaywright`
- or relaunch with `pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1`

### Window is visible but automation attaches to nothing

Cause:

- wrong port
- wrong target
- stale instance without CDP still running

Fix:

- check `http://127.0.0.1:9333/json/version`
- check `http://127.0.0.1:9333/json/list`
- close the old instance and relaunch with Playwright enabled

### Visible window becomes broken or invisible when Playwright mode is enabled

Cause:

- WebView2 profile conflict between normal mode and CDP mode

Fix:

- keep the current separate Windows WebView2 data-directory behavior from `src-tauri/src/lib.rs`
- if this regresses, verify that Playwright mode still uses `EBWebView-playwright-<port>` and not the normal `EBWebView` profile

## Related Docs

- [[TESTING]]
- [[.AGENTS/USERs_BUILD_FUNCTIONS|USERs_BUILD_FUNCTIONS.md]]

