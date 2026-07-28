# CLI Text-to-Speech File Conversion

Use AivoRelay's saved **Text to Speech** configuration to create MP3 or WAV
audio from a text or Markdown document.

## Basic commands

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3
AivoRelay.exe --convert-file .\notes.txt --output .\notes.wav
```

`.md` and `.txt` are first-class inputs. Markdown is converted to readable
text before TTS-only preprocessing, semantic chunking, retry, and synthesis.
The output extension selects MP3 or WAV. If `--output` is omitted, the saved
TTS output format is used beside the input file.

The CLI uses the provider, API-key source, voice, model, speed, preprocessing,
chunking, retry, pauses, output bitrate, and defaults saved under
**Settings → Text to Speech**. When TTS History capture is enabled, every
successful CLI text-to-audio conversion also receives a managed History copy.
Failure to create that secondary copy is reported without discarding the
successfully created output file.

## Named voice prompts

TTS voice-instruction presets are stored separately from AI/LLM
post-processing prompts. Save and select them on the Text to Speech page, then
call one by name:

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3 --tts-prompt "Calm narrator"
```

For a short one-off OpenAI voice instruction, PowerShell single quotes keep
the text literal:

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3 --tts-instructions 'Speak calmly; preserve acronyms.'
```

For long or reusable instructions, avoid shell quoting and Windows
command-line length limits by using a UTF-8 file:

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3 --tts-instructions-file .\narration-instructions.txt
```

On Windows, `cmd.exe` limits the complete expanded command line to 8,191
characters. Direct Win32 process creation allows at most 32,767 characters,
including the terminating null, but the actual shell or launcher may impose a
smaller limit. Prefer `--tts-instructions-file` well before either boundary so
the prompt is not truncated or damaged by shell quoting. See Microsoft's
[command-prompt limit](https://learn.microsoft.com/en-us/troubleshoot/windows-client/shell-experience/command-line-string-limitation)
and
[`CreateProcess` limit](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessa).

Instruction precedence is:

1. `--tts-instructions-file`
2. `--tts-instructions`
3. `--tts-prompt`
4. the selected/saved TTS instructions

Voice instructions require OpenAI TTS and a compatible
`gpt-4o-mini-tts` model. An explicitly requested CLI prompt is rejected before
the provider request if the current model is incompatible. Saved instructions
remain stored but inactive for OpenAI models that do not support instructions.
OpenAI voice instructions are limited to 4,096 characters; the same validation
applies to inline text, instruction files, named presets, saved instructions,
and history regeneration. Argument text is passed as data and is never
evaluated as shell code.

## Progress and automation

Chunk and retry progress is written to the current terminal. For scripts, add
`--json`; progress is suppressed and stdout contains one final JSON object.

Existing output files are never overwritten. Disk capacity is checked before
and during conversion, and the completed file is published only after every
chunk and final encoding succeed.

Successful provider chunks are checkpointed as verified PCM. If the provider,
network, app, or computer fails, repeating the same conversion to the same
output path automatically recovers compatible chunks instead of paying to
synthesize them again. The terminal and `--json` result report
`resumed_chunks`. Changing the source text, provider, active model/voice,
voice instructions, speed, chunk plan, or pauses starts safely from zero;
retry count, API-key source, disk/history limits, final MP3 bitrate, and
MP3/WAV selection do not invalidate compatible PCM.

Ordinary conversions keep a resume sidecar workspace beside the requested
output so temporary PCM stays on the destination disk. Managed-only History
regeneration uses an app-cache workspace with a stable History-entry key.
Explicit **Cancel** deletes the checkpoint; runtime/provider failures retain
it. Managed-only regeneration also retains complete resumable PCM until the
new History row and managed audio copy are safely stored. A final MP3/WAV is
still encoded from the complete verified PCM and published atomically—an
incomplete file is never presented as finished.

See also [[CLI-SPEECH-TO-TEXT|CLI Speech-to-Text File Conversion]].

## TTS History CLI

TTS History is separate from transcription history. Existing records remain
available to the CLI even when new-history capture is disabled.

```powershell
AivoRelay.exe tts-history list
AivoRelay.exe tts-history list --limit 20
AivoRelay.exe tts-history list --group "source-group-id"
AivoRelay.exe tts-history show 42
```

`list` is newest-first. `show` includes the retained raw source text, provider,
model, voice, output format, prompt metadata, and whether the managed audio
copy still exists.

Exporting copies retained audio and does **not** make an API request:

```powershell
AivoRelay.exe tts-history export 42 --output .\retained-copy.mp3
```

The output extension must match the retained MP3/WAV format. Existing files
are never overwritten.

### Regenerate a comparison variant

Regeneration makes a **new paid TTS API request**. The source result and every
older variant remain unchanged; the successful result is appended under the
same comparison-group ID. Retained Markdown remains Markdown and passes through
the same Markdown-to-readable-speech normalization as the original conversion.

```powershell
# Keep only a new managed History result (default MP3 at 256 kb/s)
AivoRelay.exe tts-history regenerate 42 --yes

# Interactive API-credit confirmation
AivoRelay.exe tts-history regenerate 42 --output .\variant.mp3

# Explicit confirmation for scripts
AivoRelay.exe tts-history regenerate 42 --output .\variant.mp3 --yes

# Use a different provider/model/voice
AivoRelay.exe tts-history regenerate 42 --output .\variant.wav `
  --provider openai --model gpt-4o-mini-tts --voice marin --format wav --yes

# MP3 CBR override
AivoRelay.exe tts-history regenerate 42 --output .\variant.mp3 `
  --format mp3 --bitrate 256 --yes
```

`--output` is optional for regeneration. Without it, AivoRelay stores only the
new managed History result; the default is MP3 at 256 kb/s. `--format wav`
selects managed WAV instead, and `--format mp3 --bitrate N` overrides the
managed MP3 bitrate. With `--output`, the output extension determines the
format and must match any explicit `--format`. Managed-only regeneration also
recovers verified chunks after a failed process and reports
`resumed_chunks`.

Supported providers are `soniox`, `deepgram`, and `openai`. Supported formats
are `mp3` and `wav`; MP3 bitrates are `64`, `96`, `128`, `192`, `256`, and
`320` kb/s. Provider credentials, retry, chunking, preprocessing, speed, and
other behavior still come from the saved Text to Speech settings.

The History limits configured in the app apply to results created by the UI,
ordinary CLI conversions, automatic folder conversion, and regeneration.
After a successful save, AivoRelay automatically removes the oldest managed
results until both the maximum result count and maximum managed-audio size are
satisfied.

Prompt overrides use the same literal-data behavior and precedence as normal
file conversion:

```powershell
AivoRelay.exe tts-history regenerate 42 --output .\variant.mp3 `
  --tts-prompt "Calm narrator" --yes

# PowerShell single quotes preserve the inline value literally.
AivoRelay.exe tts-history regenerate 42 --output .\variant.mp3 `
  --tts-instructions 'Speak calmly; say $HOME literally.' --yes

# Best for long instructions and Windows command-line limits.
AivoRelay.exe tts-history regenerate 42 --output .\variant.mp3 `
  --tts-instructions-file .\narration-instructions.txt --yes
```

Precedence is instructions file, inline instructions, named preset, then the
retained/saved prompt. Prompt instructions require OpenAI and a compatible
`gpt-4o-mini-tts` model.

### Delete retained history

```powershell
# Interactive destructive confirmation
AivoRelay.exe tts-history delete 42

# Explicit confirmation for scripts
AivoRelay.exe tts-history delete 42 --yes
```

Deletion removes only the history database record and its managed retained
audio. It never deletes an external export or the original user output file.
If the database row is removed but managed-audio cleanup is missing or fails,
the CLI reports a partial-deletion error instead of silently claiming complete
success.

### JSON and exit codes

`--json` may follow any executed history operation and produces exactly one
JSON object on stdout. Human progress and confirmations are suppressed. Clap's
`--help` and `--version` displays remain human-readable and exit before an
operation runs. Paid regeneration and deletion require `--yes` in JSON or
non-interactive contexts.

| Code | Meaning                                                                                                      |
| ---: | ------------------------------------------------------------------------------------------------------------ |
|  `0` | Success, including an empty history list                                                                     |
|  `1` | Runtime, provider, credential, database, or filesystem failure                                               |
|  `2` | Invalid arguments or settings override                                                                       |
|  `3` | Required destructive/API-credit confirmation was not provided                                                |
|  `4` | History result or group ID not found                                                                         |
|  `5` | Output collision; no overwrite was attempted                                                                 |
|  `6` | Retained managed audio is missing                                                                            |
|  `7` | Partial completion, such as row deletion without audio cleanup or generated audio without a retained variant |
