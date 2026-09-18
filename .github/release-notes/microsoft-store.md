<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Fixed live dictation (Soniox, Deepgram, OpenAI Realtime, Gemini Live) not restoring your clipboard after Stop: whatever you had copied before, including images and formatted text, is put back again, and the full transcript is offered to the clipboard where your settings allow it. This had been broken since 1.0.38.
- The setup wizard can now configure Gemini 3.5 Transcribe (connection route, live or after-recording workflow), and the wizard can be skipped to go straight to the main menu.
- AivoRelay now opens even when Windows blocks microphone access: Speech / Microphone settings show a reminder with a button to open Windows privacy settings, and the wizard's permission step can be skipped.
- The decapitalize indicator on the recording overlay is smaller by default, can be sized from 6 to 48 px, and its appearance settings now live in their own card under User Interface with links to and from the Decapitalize After Manual Edit settings.
- Collapsing the Recording Overlay settings section hides the floating preview, the Live Preview section can be collapsed too, and the Custom Position card no longer covers the Overlay Position label.
- The Gemini Live early finalization delay is now called "Wait for text for this long after Stop" and explains what shorter and longer values trade off.

---

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.

Optional for GitHub builds: users can install AivoRelay's self-signed root certificate so Windows trusts packages from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md). Microsoft Store installation does not require it.
