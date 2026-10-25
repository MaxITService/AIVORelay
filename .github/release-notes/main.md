<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Experimental Linux x64 and macOS Apple Silicon builds, with automatic updates.
- Settings search finds settings by what you want to do, not only by their names.
- File transcription and file-to-speech batches are saved queues that survive a restart, retry while the engine is busy, and keep earlier results.
- Optionally queue a recording started while the previous one is still being processed.
- Remove unused local speech-to-text and text-to-speech models to free disk space; alternate quantizations already on disk are recognized.
- The first-start wizard shows each model's quantization and suggests asking an AI chat for setup help.
- The read-aloud overlay applies the playback effect to all audio and can seek into parts not yet decoded.
- Fixed streaming transcription text landing in the wrong recording, the end of resampled audio being cut off in file transcription, and push-to-talk mode being lost for queued recordings.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often. Needs a processor with AVX2.
- **Store Edition files in this release** (`aivorelay-store_1.0.51_x64_en-US.msi`, `aivorelay-store_1.0.51_x64_portable.zip`): the Store build for installing without the Store. Needs AVX2 and does not update itself.
