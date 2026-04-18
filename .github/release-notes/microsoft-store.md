<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Added a visible CPU acceleration warning for systems that may need the Store build fallback.
- File transcription can now run while recording is in progress, with shorter bounded Cohere chunks for long files.
- Added pause or mute while recording options.
- Improved history controls, including a delete-all action and clearer history settings layout.
- Added AWS Bedrock (Mantle) as a built-in post-processing provider.
- File transcription cancellation now gives clearer feedback and handles cancellation more reliably.

---

This is a specialized build for the Microsoft Store Edition. It has auto-updates disabled and includes Store-specific branding.

**Notice:**
This branch remains the recommended fallback if the standard Windows build crashes on an older CPU during transcription.
