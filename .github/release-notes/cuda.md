<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

This is a specialized CUDA build for NVIDIA systems.

It is built from the `cuda-integration` branch using the AIVORelay dependency forks for `transcribe-rs` and `whisper-rs`.

## Highlights

- Whisper acceleration can now list detected GPUs and let you choose a specific device directly.
- The app now shows a clearer message when no microphone or other audio input device is available.
- Settings dropdowns now open upward to reduce overlap and clipping issues in the settings UI.
