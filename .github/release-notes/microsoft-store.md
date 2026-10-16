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

This is the Microsoft Store Edition. Installation and update delivery are handled through Microsoft Store, so AivoRelay's GitHub self-update endpoints and updater artifacts are disabled in this build.

**Notice:**
The current Store package targets Windows x64 and requires an AVX2-capable processor. It is not a compatibility fallback for processors without AVX2.
