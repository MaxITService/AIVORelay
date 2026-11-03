<!--
Update this file before publishing when you want branch-specific lead-in notes.
GitHub Actions prepends this Markdown above GitHub-generated release notes.
-->

### Dictation in several languages

- In **Speech / Microphone**, choose the languages you speak and use **Auto** for transcription. A separate downloadable language detector can select between them before supported local models transcribe a completed recording. This helps when you alternate between languages instead of selecting one manually before every recording.
- You can choose when the separate detector is used and how much speech it examines. Live streaming still uses the recognition model's own language detection or the configured fallback. History retains the detected language and how the language was selected.
- Switch the languages of the active transcription profile from the tray menu.
- Traditional Chinese output now offers regional variants for Taiwan and Hong Kong, selectable in your transcription profile.

### Easier first-time setup

- The setup wizard compares model language coverage and recognition scores for your chosen languages, then recommends suitable local models. It remembers those languages and can resume after an interruption.
- An optional dictation exercise lets you check your microphone and shortcut before working in another app. If the microphone is silent, the wizard helps you choose another input device. It also flags slow recognition and guides macOS users through the required permissions.
- Model downloads now show an estimate of the time remaining, alongside download progress.

### Audio files and History

- Drag an audio file into the app from another settings page to open file transcription directly.
- Save a completed transcript and its audio to **History**, then follow the link to the saved entry to review the text or play the recording. For a batch, you can enable automatic History capture for each completed file.
- Queued files wait and retry when a recording is using the local recognition engine. If the app view reloads during conversion, the interrupted file is marked as failed rather than silently starting over, so you can decide whether to retry.

### Read-aloud and other fixes

- The read-aloud seek bar remains usable while playback restarts after a jump, allowing you to adjust the position again immediately.
- New Cartesia configurations and built-in presets use **Sonic 3.6**. Murf configurations explicitly select **Falcon 2.1**; Murf's earlier Falcon 2 identifier already routes to this model. Existing saved selections and presets are not overwritten, and earlier supported model IDs remain selectable.
- The same hotkey can no longer be assigned to two actions, including actions whose feature is turned off (for example, AI Replace and Send to Extension). On update, a conflicting shortcut that could not register is cleared with a notification so you can assign another one.
- Click an error message to dismiss it. Active warnings in the tray and footer clear automatically after 30 seconds, while their messages remain in session history.
- The model-list refresh button gently pulses until the server list is loaded. Its tooltip explains how to load the list and search for a model.
- Fixed AI Replace triggering apps that react to a double Ctrl press (such as Obsidian's Double Shift plugin) when it cut the selected text.
- Fixed saved API keys being reported as missing on Linux and macOS, and corrected Linux/macOS packaging dependencies.
- Included upstream Handy improvements and fixes.

---

**Notice:**
**Linux and macOS builds are experimental and untested. Use them at your own risk: system-wide failures are possible.** Linux needs glibc 2.39 or newer (Ubuntu 24.04 or later). The macOS build runs on Apple Silicon only and is not notarized, so macOS may ask you to grant Accessibility and microphone access again after an update.

**Which Windows version to choose:**
- **This GitHub release** (`AivoRelay_1.0.52_x64_en-US.msi`) gets new versions first and updates itself. It is signed with our own certificate, so Windows may show SmartScreen or publisher warnings; to avoid them, you can install the optional [root certificate](https://github.com/MaxITService/AIVORelay/blob/main/docs/WINDOWS-CERTIFICATE-INSTALLATION.md).
- **[Microsoft Store](https://apps.microsoft.com/detail/9ppfkfh2zn1l)**: no SmartScreen warning, and the Store updates it, but new versions arrive less often.
- **Store Edition files in this release** (`aivorelay-store_1.0.52_x64_en-US.msi`, `aivorelay-store_1.0.52_x64_portable.zip`): the Store build for installing without the Store. It does not update itself.
