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

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.

Optional for GitHub builds: users can install AivoRelay's self-signed root certificate so Windows trusts packages from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md). Microsoft Store installation does not require it.
