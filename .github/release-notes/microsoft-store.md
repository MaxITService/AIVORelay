<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Added a beta speech-only mode that runs dictation without the full interface to reduce memory use, with clear recovery through the tray.
- Transcription profiles can use their own models and activate automatically for matching applications, window titles, or paths.
- Profile LLM post-processing can now copy any saved prompt into a profile-specific override.
- Regular dictation, audio files, and Live Monitor can use independent models, with improved speaker handling and timed transcript export.
- Added comprehensive Text to Speech workflows for selected text and file conversion, including resumable work and optional history.
- Expanded Help and settings search, including direct navigation and better discovery across interface languages.
- Improved security and reliability across audio capture, Gemini sessions, model downloads, clipboard handling, startup, history, and the interface.
- Included the latest upstream Handy improvements and fixes.

---

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.

Optional for GitHub builds: users can install AivoRelay's self-signed root certificate so Windows trusts packages from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md). Microsoft Store installation does not require it.
