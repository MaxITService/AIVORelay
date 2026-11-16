<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

[Download for Windows](https://github.com/MaxITService/AIVORelay/releases/download/v1.0.54/00_AivoRelay_1.0.54_x64_en-US.msi)

### What's new

- Added **ElevenLabs Scribe v3 Live** for live dictation and **Live Monitor** with speaker separation. **Transcribe File** uses Scribe v3 and supports text and SRT/VTT subtitles.
- Improved **Gemini live dictation** when clearing text, stopping, cancelling, and receiving the final words of a recording.
- **Decapitalize after manual edit** can be triggered repeatedly during one live recording, including across separate transcript chunks.
- Improved protected-window output and clipboard restoration. Text waiting for the original window is preserved, and cancellation prevents queued text from being inserted later.
- When automatic model fallback is enabled, an unavailable selection can switch to a recently used, downloaded local model with an explanatory notice.
- Model cards show live, file, subtitle, and speaker-separation capabilities. Optional profile settings now have a neutral hint instead of an error-colored warning.
- Store Edition consistently disables self-updates and portable data-folder controls, including when installed outside the Microsoft Store.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** (`00_AivoRelay_1.0.54_x64_en-US.msi`) gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often.
- **Store Edition files in this release** (`aivorelay-store_1.0.54_x64_en-US.msi`, `aivorelay-store_1.0.54_x64_portable.zip`): the Store build for installing without the Store. It does not update itself. Its data stays in this computer's standard per-user folders, including when launched from the ZIP.
