<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Added a small built-in voice gallery with representative samples from several cloud and local providers; applying a sample configures its provider, model, voice, language, and voice controls.
- Added compact Opus output for text-to-speech and file conversion, including bundled Opus support that does not depend on a system installation.
- Added configurable early finalization for Gemini live dictation to reduce perceived latency.
- Expanded settings search across the complete interactive and file TTS interface, including navigation into collapsed sections, and completed the global shortcut guide for read actions.
- Fixed Voice Commands so inherited models are used consistently, prompt values are inserted literally, and English-only Soundex matching is skipped for unsupported scripts such as Cyrillic.
- Improved native region capture bounds, audio capture, model availability, recording-overlay scaling, and gapless TTS playback.
- Hardened recording, file transcription, history, hotkeys, clipboard streaming, TTS queues, and browser connector state against races, stale data, and failed persistence.
- Included the latest upstream Handy transcription improvements and fixes.

---

**Notice:**
If the application crashes on an older CPU during transcription, use the Microsoft Store build when a matching Store release is available for this version.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
