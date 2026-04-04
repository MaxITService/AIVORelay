<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Fixed loading of partially invalid settings so the app recovers more reliably from broken config state.
- Added Cohere transcription support.
- Model settings now show more metadata, including supported languages.
- Recording overlay customization is expanded with icon and decap indicator controls.
- Added a floating overlay preview panel and reorganized overlay appearance settings for easier setup.
- Improved local model warmup by preloading the recorder.
- Increased the default model unload timeout to 15 minutes.
- Added a fix for older CPUs that could crash during transcription.
- The tray tooltip now shows the current app version.

---

**Notice:**
If the application crashes on an older CPU during transcription, please use the Microsoft Store build for the same app version: [v1.0.8-store](https://github.com/MaxITService/AIVORelay/releases/tag/v1.0.8-store).
