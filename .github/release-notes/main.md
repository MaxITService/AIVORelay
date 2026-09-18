<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Fixed live dictation (Soniox, Deepgram, OpenAI Realtime, Gemini Live) not restoring your clipboard after Stop: whatever you had copied before, including images and formatted text, is put back again, and the full transcript is offered to the clipboard where your settings allow it. This had been broken since 1.0.37.
- The setup wizard can now configure Gemini 3.5 Transcribe (connection route, live or after-recording workflow), and the wizard can be skipped to go straight to the main menu.
- AivoRelay now opens even when Windows blocks microphone access: Speech / Microphone settings show a reminder with a button to open Windows privacy settings, and the wizard's permission step can be skipped.
- The decapitalize indicator on the recording overlay is smaller by default, can be sized from 6 to 48 px, and its appearance settings now live in their own card under User Interface with links to and from the Decapitalize After Manual Edit settings.
- Collapsing the Recording Overlay settings section hides the floating preview, the Live Preview section can be collapsed too, and the Custom Position card no longer covers the Overlay Position label.
- The Gemini Live early finalization delay is now called "Wait for text for this long after Stop" and explains what shorter and longer values trade off.

---

**Notice:**
If the application crashes on an older CPU during transcription, use the [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.41-store) for this version.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
