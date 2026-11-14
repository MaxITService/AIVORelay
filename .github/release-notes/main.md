<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

### What's new

- Added **Reson8 Rumble-2** for live dictation, Live Monitor, and audio file transcription, with supported language choices. Live output inserts finalized chunks without an interim preview; file output is plain text, without SRT/VTT subtitle timestamps.
- Choosing a cloud model in a custom transcription profile now changes only that profile. API-key controls explain when shared settings are needed and link to the Default profile settings without changing the active profile.
- Portable users can choose **Advanced → Portable Data Storage** to keep data beside the app or in this computer's standard user folder. Restart to apply the choice; existing files are never moved or merged. Windows API keys remain in Windows Credential Manager; choosing another key-storage location is planned for a future version.
- Portable update prompts explain that the MSI installs or updates the desktop copy without updating the portable folder or importing its data.
- **Transcribe File**, including saving its audio to **History**, now opens `.opus`, `.oga`, Ogg Opus, and WebM Opus recordings directly.
- Added **ElevenLabs v4 Turbo** in **Speak selected text** and **Text file to MP3**, with voice controls and inline audio tags. Fixed an ElevenLabs request address issue.
- With a clipboard paste method, an **experimental, off-by-default** option can leave the finished transcript on the clipboard when Windows blocks insertion into an administrator window. An in-app notice explains how to paste with `Ctrl+V`; enabling this option overrides clipboard restoration for that fallback.
- Improved Linux `Shift+Insert` support, selection restoration, paste-tool fallback, overlay positioning, and input-tool detection. Improved macOS permission rechecks.
- Added clearer missing-model and silent-microphone notices. Local recognition ignores punctuation-only results, can recover after graphics errors, and handles model downloads through filtering proxies more reliably.
- Fixed wrapping cloud-model buttons and disabled the recording overlay's browser context menu in production builds.
- Included the latest upstream Handy improvements and fixes.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** (`AivoRelay_1.0.53_x64_en-US.msi`) gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often.
- **Store Edition files in this release** (`aivorelay-store_1.0.53_x64_en-US.msi`, `aivorelay-store_1.0.53_x64_portable.zip`): the Store build for installing without the Store. It does not update itself.
