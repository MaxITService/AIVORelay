<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Save your own named overlay presets and share the complete appearance through style codes. Previously shared codes still import.
- Give the overlay a new look with 3D-style animated bars, backgrounds, centerpieces, and materials.
- Choose new status icon frames and optionally make the cancel button invisible while keeping it clickable.
- Overlay presets, reset, and imported styles now save the complete appearance together and report any saving errors.
- Preview the overlay while capture starts, during silence, or with an application name. Previews also respect reduced-motion settings.
- Before installing a local speech model, you can review its download sources, installation details, and component licenses, then confirm that you have read them.
- Soniox text to speech now uses the v2 model by default, including the built-in voice presets.
- Removing filler words preserves sentence capitals and English names such as Ha Long Bay.
- Local recording now explains when the selected model is missing or unavailable instead of starting a recording that cannot be transcribed.
- Windows builds now carry a signing timestamp, so their digital signatures stay valid long-term, including after the signing certificate is renewed.
- Included the latest upstream Handy improvements and fixes for text cleanup, recording, and startup.

---

**Notice:**
The [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.45-store) is also available for this version. It targets Windows x64 and requires an AVX2-capable processor.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. The install script now limits the certificate to code signing, the same way Windows limits its built-in certificates; if you installed it before, you can run the new script once to apply this. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).

The [Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l) edition avoids the Windows SmartScreen warning (“Windows protected your PC”) shown before running downloaded files.
