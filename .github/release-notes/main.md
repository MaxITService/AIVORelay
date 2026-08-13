<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Added an optional in-memory **Listen Later** queue for clipboard and selected-text speech, with source labels, drag-and-drop and keyboard reordering, and skip, remove, and clear controls.
- Made queued speech resilient when existing audio is playing, the queue is cleared or disabled, playback fails, or the overlay must resize on a small display; read-request problems now remain visible even when the main window is hidden.
- Added the missing LLM Post-Processing status and controls to the **Default (Global)** transcription profile, including the realtime-output warning and a direct link to shared LLM configuration.
- Fixed Text to Speech conversions that could stop showing progress when a provider omitted progress updates.
- No new upstream Handy intake is included since v1.0.30.

---

**Notice:**
If the application crashes on an older CPU during transcription, use the Microsoft Store build when a matching Store release is available for this version.
