<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Fixed a rare live transcription issue that could paste previous clipboard contents instead of the latest transcribed text.
- Reduced memory use with remote transcription by loading local transcription components only when they are needed.

---

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.
