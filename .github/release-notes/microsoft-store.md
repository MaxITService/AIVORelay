<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

## Highlights

- Remote STT now explains missing Custom API keys clearly in transcription and test-connection flows.
- Remote API labels now distinguish Groq, GPT Realtime, translate mode, and custom endpoint hosts.
- Custom Remote API defaults are OpenAI-compatible without rewriting existing custom endpoints.
- Preview insert with post-processing now shows a clear processing state and keeps the preview open on failure.
- Settings repair is safer for renamed preview options and backs up the settings store before automatic repair.

---

This is a specialized build for the Microsoft Store Edition. It has auto-updates disabled and includes Store-specific branding.

**Notice:**
This branch remains the recommended fallback if the standard Windows build crashes on an older CPU during transcription.
