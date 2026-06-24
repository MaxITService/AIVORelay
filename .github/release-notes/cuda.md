<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Each recording now starts with a clean resampler state, preventing audio samples from carrying over between sessions.
- Debug settings now include an in-session notification history for easier troubleshooting.
- Onboarding errors are now shown reliably instead of being silently missed.
- Empty or whitespace-only transcriptions no longer trigger unnecessary LLM post-processing requests.
- Improved transcription shutdown resilience and reduced recording-overlay background work when the overlay is disabled.

---

This is a specialized CUDA build for NVIDIA systems.

It is built from the `integration/cuda` branch using the AIVORelay dependency forks for `transcribe-rs` and `whisper-rs`.
