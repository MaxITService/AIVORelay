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

Both launch paths reject an occupied CDP port before the build begins. This catches the common case where an older AivoRelay instance is still listening before it turns into a confusing single-instance or attach failure.

## What The Launcher Does

`scripts/start-playwright-tauri-dev.ps1`:

- loads the Rust/MSVC environment through `scripts/setup-rust-build-env.ps1`
- sets `CARGO_TARGET_DIR` to a short Windows-safe path
- ensures `src-tauri/vulkan-1.dll` exists
- rejects an already occupied loopback CDP port
- sets `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT`
- starts `bun x tauri dev`

## Verify That CDP Is Open

After the app starts, this should return JSON:

```powershell
Invoke-WebRequest -UseBasicParsing http://127.0.0.1:9333/json/version | Select-Object -ExpandProperty Content
```

If that endpoint is not available, Playwright will not be able to attach.

For an end-to-end check through Playwright itself, run this from a second PowerShell:

```powershell
Test-AivoRelayPlaywright
```

The checked-in equivalent works without loading the user's PowerShell profile:

```powershell
python .\scripts\check-playwright-tauri.py
```

An optional screenshot confirms which visible window was attached:

```powershell
Test-AivoRelayPlaywright -ScreenshotPath .\aivorelay-playwright.png
```

The checker exits nonzero when CDP is unavailable, the main AivoRelay target is missing, or the React root is not visible.

For the full TTS voice-gallery E2E check, run:

```powershell
Test-AivoRelayTtsGallery
```

The checked-in equivalent is:

```powershell
python .\scripts\check-playwright-tts-gallery.py
```

This scenario opens both TTS pages, proves the gallery starts collapsed, checks
all manifest cards and Opus previews, applies one voice, and verifies that the
output format and both bitrate settings stay unchanged. It snapshots and restores
the original TTS settings even when an assertion fails.

## Connect From Playwright

Minimal Playwright attach example:

```ts
import { chromium } from "playwright";

const browser = await chromium.connectOverCDP("http://127.0.0.1:9333");
const context = browser.contexts()[0];
const page = context
  .pages()
  .find((candidate) => new URL(candidate.url()).pathname === "/");

if (!page) throw new Error("AivoRelay main window was not found");

console.log(await page.title());
```

If you want to inspect the available CDP targets first:

```powershell
Invoke-WebRequest -UseBasicParsing http://127.0.0.1:9333/json/list | Select-Object -ExpandProperty Content
```

The local checker uses Python Playwright. Install or update it with:

```powershell
python -m pip install --upgrade playwright
```

No Playwright-managed browser download is required for this flow because Playwright attaches to the WebView2 runtime already hosting the Tauri window.

## Important Implementation Notes

- The app is single-instance through `tauri-plugin-single-instance`.
- Do not plan around "one visible instance for the user and another for Playwright".
- The correct model is one shared instance with CDP enabled.
- `src-tauri/src/lib.rs` is the source of truth for the main window and WebView2 browser args.
- Prefer `data-testid` selectors for actions and `#root` or
  `#tts-voice-gallery` for stable containers. The TTS gallery exposes stable
  sidebar, toggle, card, audio, Apply, output-format, and bitrate selectors.
  Translated button text can change, and a SettingsGroup toggle's accessible
  name includes its title and description.

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
