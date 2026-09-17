<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Gemini Live Debug Capture now records session activity and errors in the Remote Debug Log, including sessions that return no text.
- Added an Early finalization option to discard Gemini Live's standalone space after Stop so it cannot replace newly selected text.
- Clarified that Initial Live Monitor mode applies only to AivoRelay's Live Monitor.

---

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.

Optional for GitHub builds: users can install AivoRelay's self-signed root certificate so Windows trusts packages from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md). Microsoft Store installation does not require it.
