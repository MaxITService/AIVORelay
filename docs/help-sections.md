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

Smart Help gives beginners three direct starting points: speak to text, clean
up text, or read text aloud. Each choice jumps to the matching Help section.

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

**Models** chooses where speech becomes text. A local model works on this
computer. A cloud provider processes the recording online.

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
playback, and History.

Console commands: selected-text reading has no direct console command in the
current CLI documentation.

## 7. Choose text files. Get MP3 or WAV audio.

Open **Text file to mp3** and choose a cloud provider, an installed Windows
voice, or an optional local model. Select a text or Markdown file, inspect its
characters and planned chunks, choose MP3 or WAV and an output path, then start
the conversion. The page also supports multiple files, folders, resumable
conversions, folder automation, and File History.

Console commands:

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3
AivoRelay.exe --convert-file .\notes.txt --output .\notes.wav
AivoRelay.exe --convert-file .\chapter.md --output .\windows.wav --tts-provider windows
```

See [[CLI-TEXT-TO-SPEECH|CLI Text-to-Speech File Conversion]] for provider
overrides, safe output handling, and local voice engines.

## 8. Speak a command. AivoRelay performs it.

**Voice Commands** lets a spoken instruction trigger an enabled app action. This
page appears only when Voice Commands is enabled.

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
