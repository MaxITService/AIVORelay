<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Overlay presets, reset, and imported styles now save the complete appearance together and report any saving errors.
- Preview the overlay while capture starts, during silence, or with an application name. Previews also respect reduced-motion settings.
- Before installing a local speech model, you can review its download sources, installation details, and component licenses, then confirm that you have read them.
- Soniox text to speech now uses the v2 model by default, including the built-in voice presets.
- Removing filler words preserves sentence capitals and English names such as Ha Long Bay.
- Local recording now explains when the selected model is missing or unavailable instead of starting a recording that cannot be transcribed.
- Included the latest upstream Handy improvements and fixes for text cleanup, recording, and startup.

---

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.

Optional for GitHub builds: users can install AivoRelay's self-signed root certificate so Windows trusts packages from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md). Microsoft Store installation does not require it.
