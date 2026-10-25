# Linux and macOS
Branch tags: #branch/main

Not compilable on the Windows dev machine; verify with the manual `Platform check` workflow (`.github/workflows/platform-check.yml`).

- Linux paste/typing tools, X11 push-to-talk release grace, layer-shell overlay, History playback: `clipboard.rs`, `shortcut.rs` (`linux_ptt`), `overlay.rs`, `HistorySettings.tsx`.
- macOS: Enigo and shortcuts are created on request (`initialize_enigo`/`initialize_shortcuts`) once Accessibility exists; `apply_startup_activation_policy` in `lib.rs`.
- Env switches: `AIVORELAY_NO_GTK_LAYER_SHELL`, `AIVORELAY_METAL_RESIDENCY`, `AIVORELAY_FORCE_AI_STUB`, `AIVORELAY_DISABLE_UPDATER` (Nix sets it), `WEBKIT_DISABLE_DMABUF_RENDERER`.
- Off Windows the post-process, AI Replace and voice-command keys stay in the settings JSON (and `get_app_settings` returns them to the webview); Soniox, Deepgram, TTS, TTS-LLM and the remote OpenAI-compatible STT key use `secure_keys` (named keys). Remote STT providers work on every platform.
- Linux CI jobs get no secrets or write token and build without updater artifacts (`tauri.linux.conf.json`); keep it that way. macOS also builds without them, so `update_checks_forced_disabled()` locks update checks off on both until a later version adds their updates.
- Nix: `flake.nix`, `.nix/bun.nix` (regenerate with `bun scripts/check-nix-deps.ts`), checked by the manual `nix build check` workflow.
- Supported targets: Linux x64 built on `ubuntu-24.04` (needs glibc 2.39+) and macOS Apple Silicon built on `macos-26`. Linux ARM and Intel Mac are only built by `build-test.yml` and are not released.
- macOS is ad-hoc signed (`signingIdentity: "-"` in `tauri.conf.json`) and not notarized; Accessibility and microphone grants may need renewing after each update (accepted).
- One updater key (pubkey in `tauri.conf.json`) serves every platform; do not rotate it.
