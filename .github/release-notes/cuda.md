<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- File transcription can now run while recording is in progress.
- Added a setup checklist and setup health check to make setup easier.
- Improved smart chunking for long local-model file transcription jobs, including Parakeet.
- Added Cohere local transcription support and richer model metadata details.
- Default CUDA-oriented accelerator behavior remains tuned for NVIDIA systems.

---

This is a specialized CUDA build for NVIDIA systems.

It is built from the `cuda-integration` branch using the AIVORelay dependency forks for `transcribe-rs` and `whisper-rs`.
