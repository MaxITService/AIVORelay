# CLI Speech-to-Text File Conversion

Use AivoRelay's saved **Transcribe Audio File** configuration to create a
plain-text or Markdown transcript from an audio file.

## Basic commands

```powershell
AivoRelay.exe --convert-file .\meeting.mp3 --output .\meeting.md
AivoRelay.exe --convert-file .\interview.wav --output .\interview.txt
```

Supported input formats are WAV, MP3, M4A, OGG, FLAC, and WebM. The output
extension must be `.md` or `.txt`. When `--output` is omitted, AivoRelay writes
Markdown beside the input file.

The command uses the provider, credentials, model, language, diarization,
chunking, and other file-transcription options saved in the app. Configure
them under **Settings → Transcribe File**.

## Progress and automation

Human-readable progress is written to the current terminal. The completed
path and a short transcript preview are printed when conversion succeeds.

For scripts, add `--json`. Status text is suppressed and stdout contains one
final JSON object:

```powershell
AivoRelay.exe --convert-file .\meeting.mp3 --output .\meeting.md --json
```

Existing output files are never overwritten. The transcript is written to a
partial file and published atomically only after transcription succeeds.

## Legacy benchmark command

`-f` / `--transcribe-file` remains the separate local-model benchmark command
for 16 kHz mono PCM WAV input. Use `--convert-file` for the app-managed,
multi-format file-transcription workflow described above.

See also [[CLI-TEXT-TO-SPEECH|CLI Text-to-Speech File Conversion]].
