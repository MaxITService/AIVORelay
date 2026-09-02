# AivoRelay Help Section Notes

This document is the plain-language source for the Help page. It explains what
each settings page does and records the console commands that are already
documented for that task. It does not add or change commands.

## Short summary

AivoRelay turns speech into text when you press a shortcut. It can clean up
that text, replace selected text, read text aloud, transcribe files, monitor
computer audio, send text to a web chat through a Chrome extension, keep
history, and control how the app behaves.

Transcription is the name for turning speech into text. A local model works on
this computer. A cloud provider processes the recording online.

## Smart Help

Smart Help gives beginners five direct starting points:

- **I want to set up my microphone and shortcut** — start the basic dictation setup.
- **I want to use a local model that turns voice into text** — keep processing on this computer.
- **I want to use an online provider that turns voice into text** — use a provider account and API key, with possible usage costs.
- **I want to read text aloud** — choose a voice and shortcut for selected text.
- **Something is not working** — open tests, logs, and diagnostic tools.

Each card explains what is needed, jumps to the matching Help section, and has a
button for the related settings page.

## Contents

1. [[#1. Press a key. Speak. Text appears.|Press a key. Speak. Text appears.]]
   1. [[#Choose a local model or cloud provider.|Choose a local model or cloud provider.]]
   2. [[#Improve microphone audio.|Improve microphone audio.]]
   3. [[#Control advanced application behaviour.|Control advanced application behaviour.]]
2. [[#2. Speak naturally. AI cleans up the result.|Speak naturally. AI cleans up the result.]]
3. [[#3. Select text. Say what to change. It is replaced.|Select text. Say what to change. It is replaced.]]
4. [[#4. Choose an audio or video file. Get text.|Choose an audio or video file. Get text.]]
5. [[#5. Play computer audio. See the words live.|Play computer audio. See the words live.]]
6. [[#6. Select text. Press a key. Hear it spoken.|Select text. Press a key. Hear it spoken.]]
7. [[#7. Choose text files. Get MP3 or WAV audio.|Choose text files. Get MP3 or WAV audio.]]
8. [[#8. Speak a command. AivoRelay performs it.|Speak a command. AivoRelay performs it.]]
9. [[#9. Connect to an LLM chat open in Chrome via the separate extension.|Connect to an LLM chat open in Chrome via the separate extension.]]
10. [[#10. Add corrections once. Apply them automatically.|Add corrections once. Apply them automatically.]]
11. [[#11. Find your previous transcriptions.|Find your previous transcriptions.]]
12. [[#12. Choose how results appear.|Choose how results appear.]]
13. [[#13. Test features and diagnose problems.|Test features and diagnose problems.]]

## 1. Press a key. Speak. Text appears.

Set a microphone and shortcut in **Speech / Mic**. Choose where speech becomes
text in **Models**: a local model works on this computer, while a cloud provider
works online.

### Choose a local model or cloud provider.

**Models** chooses where speech becomes text.

Local models run on your computer. They usually do not need an online account or
usage payments, but they need enough disk space, memory, and processing power.

Cloud/API providers run online. You usually need to sign in to the provider's
website or console, enable the service, create an API key, and usually add a
payment method or credits. Cloud providers may charge for each request. They can
have free plans with limits.

AivoRelay does not control provider prices. Check the provider's pricing and
usage limits before using it.

⚠️ **Warning:** Never share your API key. It is a secret code that can let
someone use your provider account and spend your money on API requests. If the
key is exposed, revoke it in the provider's console and create a new one.
Software or network problems can also cause API credits to be used unexpectedly.
Avoid adding large amounts of money to an API account, and set spending limits
or usage caps wherever the provider allows them.

Console commands: no direct command is documented for changing this setting.

### Improve microphone audio.

**Speech Processing** changes the microphone input before transcription. Use it
when speech is too quiet or background noise is distracting.

Console commands: no direct command is documented for these settings.

### Control advanced application behaviour.

**Advanced** contains less common controls. Use it to change startup, clipboard,
or other application behaviour.

Console commands: no direct command is documented for these settings.

## 2. Speak naturally. AI cleans up the result.

An LLM is an AI text editor. **LLM Post Processing** can fix wording after
speech becomes text.

Console commands: this page has no direct console command in the current CLI
documentation.

## 3. Select text. Say what to change. It is replaced.

Select text, then say what you want changed. **AI Replace** returns replacement
text for the selection.

Console commands: this page has no direct console command in the current CLI
documentation.

## 4. Choose an audio or video file. Get text.

Choose an audio or video file. **Transcribe File** creates a text transcript you
can save as Markdown or plain text.

Console commands:

```powershell
AivoRelay.exe --convert-file .\meeting.mp3 --output .\meeting.md
AivoRelay.exe --convert-file .\interview.wav --output .\interview.txt
AivoRelay.exe --convert-file .\meeting.mp3 --output .\meeting.md --json
```

See [[CLI-SPEECH-TO-TEXT|CLI Speech-to-Text File Conversion]] for input types,
progress, and output rules.

## 5. Play computer audio. See the words live.

Play audio from your computer. **Live Monitor** writes the words while the audio
plays.

Console commands: no direct command is documented for the live monitor page.

## 6. Select text. Press a key. Hear it spoken.

Open **Speak selected text** and choose a cloud provider, an installed Windows
voice, or an optional local model. Start with the recommended preset, then
assign a keyboard shortcut under **Actions**. Select text in any application and
press the shortcut to hear it aloud; optional settings control the voice, speed,
playback, and History. You can read existing clipboard text, copy the selection
temporarily and then read it, or read a selection directly on supported Windows
apps.

Cloud providers send text online. Windows voices and local TTS models keep speech
generation on this computer, but optional LLM cleanup can still send text online.

Console commands: selected-text reading has no direct console command in the
current CLI documentation.

## 7. Choose text files. Get MP3 or WAV audio.

Open **Text file to mp3** and choose a cloud provider, an installed Windows
voice, or an optional local model. Select a text or Markdown file, inspect its
characters and planned chunks, choose MP3 or WAV and an output path, then start
the conversion. The page also supports multiple files, folders, resumable
conversions, folder automation, and File History. File Operations has its own
settings; saved synthesis presets can also be shared with **Speak selected text**.

Cloud providers send file text online. Windows voices and local TTS models keep
speech generation on this computer, but optional LLM cleanup can still send text
online.

Console commands:

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3
AivoRelay.exe --convert-file .\notes.txt --output .\notes.wav
AivoRelay.exe --convert-file .\chapter.md --output .\windows.wav --tts-provider windows
```

See [[CLI-TEXT-TO-SPEECH|CLI Text-to-Speech File Conversion]] for provider
overrides, safe output handling, and local voice engines.

## 8. Speak a command. AivoRelay performs it.

**Voice Commands** is a Windows-only experimental feature. The Help entry stays
visible while the feature is off so you can find these instructions.

To turn it on:

1. Open **Settings → Debug**.
2. Open **Experimental Features → Voice Commands**.
3. Read the warning and accept the risk confirmation.
4. Open **Voice Commands** from the sidebar and turn on **Enable Voice Commands**.

⚠️ **Warning:** Voice Commands can execute arbitrary PowerShell scripts or
commands from voice input. A wrong or malicious trigger could change or delete
data, damage the system, or create a security problem. Enable it only if you
understand PowerShell and review every command.

Console commands: no direct command is documented for configuring voice
commands.

## 9. Connect to an LLM chat open in Chrome via the separate extension.

The **Connector** sends text from AivoRelay to a web chat, such as ChatGPT or
Claude, open in Chrome through a separate extension. Open Connector to set up
that connection.

Console commands: no direct command is documented for the connector page.

## 10. Add corrections once. Apply them automatically.

Add a replacement such as a common spelling fix. **Text Processing** applies it
when matching text appears.

Console commands: no general text-processing command is documented for this
page.

## 11. Find your previous transcriptions.

**History** keeps previous transcriptions and recordings. Open it when you want
to find an older result.

Console commands: no direct command is documented for transcription History.

## 12. Choose how results appear.

**User Interface** controls how the overlay and results look. These settings
change what you see while using AivoRelay.

Console commands: no direct command is documented for this page.

## 13. Test features and diagnose problems.

**Debug** contains tests, logs, and diagnostic tools. Open it when a feature does
not behave as expected.

Console commands:

```powershell
AivoRelay.exe --help
AivoRelay.exe --version
```

These commands show the available CLI help and installed version. They do not
change settings.
