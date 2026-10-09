<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Local transcription runs separately from the app, improving responsiveness and recovery from speech-engine failures.
- The recording overlay and live preview clearly show when a speech model is loading.
- Choose simplified or traditional Chinese text, with separate preferences for transcription profiles.
- Fixed Soniox live-transcription connection authentication.
- New custom sounds appear when you reopen the sound selector.
- Portable installations can reuse models from earlier download locations.
- The tray briefly shows an error icon and explanation after failures.
- Settings show the correct default values.
- Included the latest upstream Handy improvements and fixes.

---

**Notice:**
The [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.47-store) is also available for this version. It targets Windows x64 and requires an AVX2-capable processor.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. The install script now limits the certificate to code signing, the same way Windows limits its built-in certificates; if you installed it before, you can run the new script once to apply this. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).

The [Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l) edition avoids the Windows SmartScreen warning (“Windows protected your PC”) shown before running downloaded files.
