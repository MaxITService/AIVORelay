<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- If the app's windows suddenly turn black because the built-in browser engine crashed (this can happen with overlay tools such as RivaTuner), AivoRelay now rebuilds its windows on its own instead of staying blank until you restart it.
- The error overlay now has a close (X) button next to Retry and Copy, so you can dismiss it right away instead of waiting for it to hide.
- A manually placed recording overlay no longer jumps or drifts when an error appears: the error layout grows in place around the recording frame, and the app's own repositioning is no longer saved as your manual position.
- The animated border styles (Traveling Highlight, Shimmer Edge, Breathing Contour) now actually move all the time, including while transcribing and in the settings preview; your voice level changes their speed and brightness.
- The Skip button in the setup wizard stays visible on every step, and the Remote STT wizard has a skip link at the bottom.
- The Recording Overlay settings remember whether the presets list was collapsed, and "Show Decapitalize Indicator In Preview" now sits at the top of the Decapitalize Indicator card.

---

**Notice:**
If the application crashes on an older CPU during transcription, use the [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.42-store) for this version.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
