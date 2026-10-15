<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Assign an optional shortcut while creating a transcription profile. Clearer guidance explains when a profile shortcut is useful and how to use Set Active or Cycle Profile instead.
- Fixed a problem that could leave the app window black when opening profile settings or changing the speech model.
- A failed settings page now shows recovery actions while other sections stay usable. Reload the window or open the log folder from the error screen; app and overlay errors are saved to the log.
- If the whole app window fails, you can also check for updates from its error screen.
- Profile output plans now name the keys being pressed and explain pauses, Preview, and what happens when the target window changes.
- What's New and profile post-processing links open the relevant settings, including controls inside collapsed cards. The text replacement rule tester is now searchable.

---

**Notice:**
The [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.49-store) is also available for this version. It targets Windows x64 and requires an AVX2-capable processor.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. The install script now limits the certificate to code signing, the same way Windows limits its built-in certificates; if you installed it before, you can run the new script once to apply this. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).

The [Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l) edition avoids the Windows SmartScreen warning (“Windows protected your PC”) shown before running downloaded files.
