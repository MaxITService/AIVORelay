<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Added an optional sound when transcribed text is ready and made audio feedback start faster while preserving the system mute state.
- Fixed cases where transcription results could disappear during text cleanup.
- Added the latest local transcription models and fixed public downloads when obsolete credentials were saved.
- Soniox live transcription now retries recoverable timeouts without duplicating already inserted text.
- Reduced memory use by sharing WebView2 resources and closing temporary interface processes after use.
- Improved Windows shutdown and x64-on-ARM reliability.
- Portable installations now link to the signed installer when an update is available.

---

**Notice:**
If the application crashes on an older CPU during transcription, use the Microsoft Store build when a matching Store release is available for this version: https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.25-store
