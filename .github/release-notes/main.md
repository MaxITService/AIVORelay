<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Removed the Browser Connector’s shared default password. AivoRelay now creates a unique connection secret and automatically provisions extension copies exported from the app.
- Made recording readiness follow the first real audio samples and prevented delayed events from an earlier recording from marking a new session ready.
- Strengthened credential handling so API keys stay out of normal logs and the webview, while provider model lists still refresh automatically for keys stored in Windows Credential Manager.
- Added an explicitly unsafe Remote STT diagnostic option for troubleshooting provider responses that echo API keys; it remains off by default.
- Improved Text to Speech, Listen Later, transcription-provider error handling, and post-processing profile behavior across several failure and transition cases.
- Added a copyable AI-chat help prompt and simplified the common-use guidance on the Help page.
- Included the latest upstream Handy improvements: recording now recovers after microphone disconnects, Custom Words accepts multi-word phrases, and portable installations keep downloaded Hugging Face models inside the `Data` folder.

---

**Notice:**
If the application crashes on an older CPU during transcription, use the Microsoft Store build when a matching Store release is available for this version.
