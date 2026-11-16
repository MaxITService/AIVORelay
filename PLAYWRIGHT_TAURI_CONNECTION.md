# Playwright + Tauri Connection

Playwright attaches over CDP to the same visible WebView2 dev window. No Playwright browser download is needed.

## Launch

- User: `Dev-AivoRelay -EnablePlaywright` or `Fast-Dev-AivoRelay -EnablePlaywright` (see [[.AGENTS/USERs_BUILD_FUNCTIONS|USERs_BUILD_FUNCTIONS.md]]).
- Agent: `pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1 [-PlaywrightPort 9334]`. Keep an interactive terminal/PTY open and wait for `http://127.0.0.1:9333/json/version` (use the chosen port). Production stays running until compilation succeeds; Ctrl+C stops this dev session and restores production, while Escape leaves production off. Do not stop a user-owned dev session.
- Both reject an occupied CDP port. The app is single-instance, so the user and Playwright share one window; do not start a second dev build.

## Check

- `python .\scripts\check-playwright-tauri.py --port 9333 --screenshot .AGENTS/.untracked/playwright/aivorelay.png` verifies the main React window and saves a PNG. Use `--timeout-ms 30000` for a slow startup.
- `python .\scripts\check-playwright-tts-gallery.py` (or `Test-AivoRelayTtsGallery`) runs the TTS voice-gallery E2E and restores TTS settings afterwards.
- Update the agent's Python package with `python -m pip install --user --upgrade playwright`; confirm with `python -m playwright --version`. Verified 2026-11-14: Playwright 1.65.2, WebView2 157.0.4341.60, CDP navigation and screenshots of Speech / Mic and Advanced.
- Direct API: `browser = await playwright.chromium.connect_over_cdp("http://127.0.0.1:9333")`; select the main page from `browser.contexts`, then `await page.screenshot(path="...")`. Closing this attached browser object disconnects Playwright; it leaves the app running.

## Rules

- The window uses the user's real settings: every click saves, and some deletes (for example a profile's trash icon) have no confirmation. Never click unlabeled icon buttons blindly.
- Prefer `data-testid` selectors; navigate with `sidebar-section-<id>`. Translated text changes, and a SettingsGroup toggle's accessible name includes its description.

## Troubleshooting

- No CDP port: the app was started without `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT`; relaunch with Playwright enabled.
- Nothing to attach to: check `/json/list` for the target and close a stale instance without CDP.
- Broken or invisible window in Playwright mode: `src-tauri/src/webview_runtime.rs` must keep the separate `EBWebView-playwright-<port>` profile (`webview-playwright-<port>` for portable data).
