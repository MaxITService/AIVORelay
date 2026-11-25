<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

[Download for Windows](https://github.com/MaxITService/AIVORelay/releases/download/v1.0.57/00_AivoRelay_1.0.57_x64_en-US.msi)

### What's new

- **Speech recognition Benchmark** lets you run the same recording through different models, compare speed and word errors, and test audio settings. Import a recording from History or record a test clip. Add the exact text under the clip or choose **Use as reference** on a result. Runs keep going when you leave Benchmark.
- **Cloud live dictation** can recognize speech as you talk and insert the finished text only after you stop. Enable it in each transcription profile. Reson8 Rumble-2 now shows words as you speak in **Live Preview** and **Live Monitor**.
- **History** can export all transcripts as JSON to a file or the clipboard.
- **Cloud speech recognition** adds connection tests for Gemini and Vercel AI Gateway. ElevenLabs Zero Retention is off by default for new configurations, and an error about it links directly to the setting. Deepgram no longer spikes CPU while finishing, and Soniox handles file cancellation and cleanup more reliably.
- **Text-to-speech** keeps voice and model changes made just before previewing or inspecting a file. Voice, language, and speed settings better match what OpenAI, ElevenLabs, Cartesia, Murf, and Deepgram accept.
- **AI post-processing** handles reasoning settings and responses more reliably across cloud providers.
- **Handy improvements** include more reliable macOS permission prompts, recording commands that work even when AivoRelay is closed, and a Linux tray icon whose visibility the desktop can remember.

Thanks again to @waLLxAck for contributing ElevenLabs v4 Turbo support, and to CJ Pais and Nemo Feng for the upstream Handy fixes.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** (`00_AivoRelay_1.0.57_x64_en-US.msi`) gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often.
- **Store Edition files in this release** (`aivorelay-store_1.0.57_x64_en-US.msi`, `aivorelay-store_1.0.57_x64_portable.zip`): the Store build for installing without the Store. It does not update itself. Its data stays in this computer's standard per-user folders, including when launched from the ZIP.
