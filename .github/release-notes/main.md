<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- LLM post-processing can now insert the answer into the target app while it streams, with target-window protection and clipboard restore. The translated LLM Benchmark shows time to first token and includes a streaming toggle.
- Gemini Live uses the selected dictation mode, waits briefly for the last words to settle after Stop, and keeps sentence spaces through Vercel AI Gateway. Background dictations count toward statistics, finish their final spacing and insertion, play the result-ready sound when no newer recording is active, and keep late words or trailing spaces out of a newly started recording.
- Profiles keep their selected speech model and Gemini overrides when saved from other settings. Clearing a custom post-processing prompt returns to the shared prompt.
- Preview keeps its text when the paste method is None. Enabling Preview disables direct live output for the profile's own model.
- History shows the text after replacements, lets you switch to the original transcript, indicates when processing left the transcript unchanged, and completes Soniox Live re-transcription without timing out.
- Custom Words preserves capitalization more reliably and no longer swallows the next word when corrections tie.
- Playback pitch changes the voice without changing its speed, including streamed speech.
- Existing app shortcuts stay suspended while a hotkey field records a new key, and clicking another shortcut field stops the previous capture cleanly.
- Screenshot folder paths expand every known environment variable even when another variable is unknown.
- New installations start hidden in the system tray by default; Start Hidden remains configurable in Advanced.
- Included the latest upstream Handy improvements and fixes: a newer local speech engine and no repeated audio when speech resumes right after a short pause.

---

**Notice:**
The [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.50-store) is also available for this version. It targets Windows x64 and requires an AVX2-capable processor.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. The install script now limits the certificate to code signing, the same way Windows limits its built-in certificates; if you installed it before, you can run the new script once to apply this. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).

The [Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l) edition avoids the Windows SmartScreen warning (“Windows protected your PC”) shown before running downloaded files.
