<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

[Download for Windows](https://github.com/MaxITService/AIVORelay/releases/download/v1.0.55/00_AivoRelay_1.0.55_x64_en-US.msi)

### What's new

- Added **ElevenLabs Scribe settings** for removing filler words and requesting that ElevenLabs not retain your data. Completed recordings and files also support numbers as digits and sound tags such as laughter. The data-retention option requires an eligible ElevenLabs plan; turn it off if your plan rejects it.
- **Custom Words** now help ElevenLabs recognize names and specialist terms in completed recordings and audio files, as well as live dictation.
- Fixed **reading selected text aloud** accidentally triggering another app's shortcut while shortcut keys were still held.
- On Windows, the window title and tray menu now show when AivoRelay is running as **Administrator**. With administrator autostart enabled, the window-title label can be hidden.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** (`00_AivoRelay_1.0.55_x64_en-US.msi`) gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often.
- **Store Edition files in this release** (`aivorelay-store_1.0.55_x64_en-US.msi`, `aivorelay-store_1.0.55_x64_portable.zip`): the Store build for installing without the Store. It does not update itself. Its data stays in this computer's standard per-user folders, including when launched from the ZIP.
