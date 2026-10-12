<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Create and duplicate transcription profiles with the same controls as Default, unique names, and a settings summary on collapsed cards.
- Each profile has its own text insertion and clipboard settings, plus separate keys for recording start, before insertion, after insertion, and cancellation.
- Protect the target window during live dictation: undelivered text waits while you switch apps. Choose how long to wait and whether to resume insertion, open Preview, or leave the text in History.
- Model selection follows the active profile and shows its settings. Model deletion gives you five seconds to undo.
- Search saved transcriptions with highlighted matches, or open them in the LLM benchmark to try another prompt.
- Preview text replacement rules on sample text and see which rules changed it, in processing order.
- The recording overlay keeps its normal size after recording stops, while speech is processed.
- More reliable microphone startup and switching to the system default microphone, an option to mute audio earlier, and smoother playback seeking.
- Start or stop recording and retrieve transcripts with explicit command-line controls.
- Use enabled Custom Words in LLM prompt templates to help preserve names and specialist vocabulary.

---

**Notice:**
The [Microsoft Store Edition](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.48-store) is also available for this version. It targets Windows x64 and requires an AVX2-capable processor.

Optional: Windows users can install AivoRelay's self-signed root certificate to trust GitHub builds from Max IT Service. The install script now limits the certificate to code signing, the same way Windows limits its built-in certificates; if you installed it before, you can run the new script once to apply this. AivoRelay also works without it, but Windows may show publisher or SmartScreen warnings. See the [certificate installation guide](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).

The [Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l) edition avoids the Windows SmartScreen warning (“Windows protected your PC”) shown before running downloaded files.
