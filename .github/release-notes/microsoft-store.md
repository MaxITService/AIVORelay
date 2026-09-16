<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Added a small built-in voice gallery with representative cloud and local voices, plus compact Opus output for text-to-speech and file conversion.
- Added configurable early finalization for Gemini live dictation to reduce perceived latency.
- Expanded settings search across the interactive and file TTS interface, including navigation into collapsed sections, and completed the global shortcut guide for read actions.
- Dictation now clearly reports when a recording ends without recognized text; very short accidental taps remain quiet, with an adjustable threshold in Debug settings.
- Improved recording, audio devices, models, file transcription, Live Monitor, Voice Commands, history, clipboard streaming, browser connector, and TTS reliability.
- The history limit field now waits until editing is finished before applying the new value.
- Included the latest upstream Handy improvements and fixes.

---

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.

Optional for GitHub builds: users can install AivoRelay's self-signed root certificate so Windows trusts packages from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md). Microsoft Store installation does not require it.
