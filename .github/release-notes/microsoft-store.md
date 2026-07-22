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

---

This is a specialized build for the Microsoft Store Edition. It has auto-updates disabled and includes Store-specific branding.

**Notice:**
This branch remains the recommended fallback if the standard Windows build crashes on an older CPU during transcription.
