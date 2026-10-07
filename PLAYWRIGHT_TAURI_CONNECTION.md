# Playwright + Tauri Connection

Playwright attaches over CDP to the same visible WebView2 dev window. No Playwright browser download is needed.

## Launch

- User: `Dev-AivoRelay -EnablePlaywright` or `Fast-Dev-AivoRelay -EnablePlaywright` (see [[.AGENTS/USERs_BUILD_FUNCTIONS|USERs_BUILD_FUNCTIONS.md]]).
- Agent: `pwsh -NoProfile -File .\scripts\start-playwright-tauri-dev.ps1 [-PlaywrightPort 9334]`. Run it in the background and wait for `http://127.0.0.1:9333/json/version`. The app lives inside this process; stop it only with the user's consent.
- Both reject an occupied CDP port. The app is single-instance, so the user and Playwright share one window; do not start a second dev build.

## Check

- `python .\scripts\check-playwright-tauri.py` (or `Test-AivoRelayPlaywright`) verifies the main window; `--screenshot <path>` is optional.
- `python .\scripts\check-playwright-tts-gallery.py` (or `Test-AivoRelayTtsGallery`) runs the TTS voice-gallery E2E and restores TTS settings afterwards.
- Python Playwright, last verified 1.63.0 on WebView2 154: `python -m pip install --upgrade playwright`.

## Rules

- The window uses the user's real settings: every click saves, and some deletes (for example a profile's trash icon) have no confirmation. Never click unlabeled icon buttons blindly.
- Prefer `data-testid` selectors; navigate with `sidebar-section-<id>`. Translated text changes, and a SettingsGroup toggle's accessible name includes its description.

## Troubleshooting

- No CDP port: the app was started without `PLAYWRIGHT_TAURI_REMOTE_DEBUGGING_PORT`; relaunch with Playwright enabled.
- Nothing to attach to: check `/json/list` for the target and close a stale instance without CDP.
- Broken or invisible window in Playwright mode: `src-tauri/src/lib.rs` must keep the separate `EBWebView-playwright-<port>` WebView2 profile.
