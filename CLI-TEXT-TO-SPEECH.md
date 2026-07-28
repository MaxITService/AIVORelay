# CLI Text-to-Speech File Conversion

Use AivoRelay's saved **Text to Speech** configuration to create MP3 or WAV
audio from a text or Markdown document.

## Basic commands

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3
AivoRelay.exe --convert-file .\notes.txt --output .\notes.wav
AivoRelay.exe --convert-file .\chapter.md --output .\windows.wav --tts-provider windows
AivoRelay.exe --convert-file .\chapter.md --output .\qwen.mp3 --tts-provider local-qwen --tts-voice Vivian
```

`.md` and `.txt` are first-class inputs. Markdown is converted to readable
text before TTS-only preprocessing, semantic chunking, retry, and synthesis.
The output extension selects MP3 or WAV. If `--output` is omitted, the saved
TTS output format is used beside the input file.

The CLI starts from **Settings → Text to Speech**, then applies any command-line
overrides to an in-memory copy. It never rewrites saved settings. When TTS
History capture is enabled, every successful CLI conversion also receives a
managed History copy. Failure to create that secondary copy is reported
without discarding the successfully created output file.

## Temporary conversion overrides

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3 `
  --tts-provider soniox --tts-model sonic-preview --tts-voice voice-id `
  --tts-language ru --tts-speed 1.2 --tts-key-source separate `
  --tts-bitrate 192 --tts-chunk-chars 1400 --tts-retries 4 `
  --tts-retry-delay-ms 750 --tts-chunk-pause-ms 80 `
  --tts-paragraph-pause-ms 300 --tts-disk-reserve-mb 1024 `
  --tts-preprocessing true --tts-history false
```

| Option | Accepted values and behavior |
| --- | --- |
| `--tts-provider` | `soniox`, `deepgram`, `openai`, `local-qwen`, or `windows` |
| `--tts-model` | Soniox, Deepgram, or OpenAI model ID |
| `--tts-voice` | Soniox/OpenAI voice ID, official Qwen speaker ID, or stable Windows voice ID; `default` selects the current Windows default |
| `--tts-language` | Soniox language code or Qwen language name |
| `--tts-speed` | Soniox `0.7–1.3`; Deepgram `0.7–1.5`; OpenAI `0.25–4.0`; Qwen/Windows `0.5–2.0` |
| `--tts-key-source` | `shared` or `separate`; chooses an already-stored cloud credential and never exposes a secret on the command line |
| `--tts-format` | `mp3` or `wav`; with `--output`, it must match the extension |
| `--tts-bitrate` | MP3 only: `64`, `96`, `128`, `192`, `256`, or `320` kb/s |
| `--tts-chunk-chars` | `50` through the selected provider's hard character limit |
| `--tts-retries` | `0–10` retries after the first attempt |
| `--tts-retry-delay-ms` | `100–30000` ms initial exponential retry delay |
| `--tts-chunk-pause-ms` | `0–5000` ms |
| `--tts-paragraph-pause-ms` | `0–10000` ms |
| `--tts-preprocessing` | `true` or `false` for saved TTS replacement rules |
| `--tts-replacements-file` | UTF-8 JSON replacement-rule array used instead of saved rules for this run |
| `--tts-disk-reserve-mb` | `0–1048576` MB minimum free-space reserve |
| `--tts-history` | `true` or `false` for this result's History capture |

The final `--json` object reports the effective provider, model, voice,
language, key source, speed, format/bitrate, chunk/retry/pause settings,
preprocessing rule count, disk reserve, and History request state.

Provider-specific options are strict. AivoRelay returns exit code `2` with an
actionable explanation instead of silently ignoring a flag or clamping its
value:

- Deepgram encodes voice and language in its model ID, so use `--tts-model`;
  `--tts-voice` and `--tts-language` are rejected.
- OpenAI does not expose a separate language option, so `--tts-language` is
  rejected.
- The local Qwen runtime uses AivoRelay's pinned model, so `--tts-model` is
  rejected; use `--tts-voice` and `--tts-language`.
- Windows derives language from its installed voice and has no API key or model
  selector, so use `--tts-voice`; model, language, and key-source flags are
  rejected.

When `--tts-provider windows` is explicit and `--tts-voice` is omitted,
AivoRelay resolves the current OS default into one stable voice ID for the
whole operation. If Windows is only inherited from saved settings, its saved
voice remains selected. Use `--tts-voice default` to force the current OS
default in either case.

The replacement file uses the same first-class rule shape as the TTS settings
page. It may contain at most 1,000 rules and 1 MiB:

```json
[
  {
    "id": "cli-ai",
    "from": "\\bAI\\b",
    "to": "A I",
    "enabled": true,
    "case_sensitive": false,
    "is_regex": true
  }
]
```

Invalid JSON, an invalid enabled regular expression, or combining the file
with `--tts-preprocessing false` fails before synthesis.

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

Voice instructions require OpenAI TTS with a compatible `gpt-4o-mini-tts`
model. An explicitly requested CLI prompt is rejected before an incompatible
provider request. Saved instructions remain stored but inactive for OpenAI
models that do not support instructions, local Qwen3-TTS 0.6B, and Windows
installed voices.
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

## Optional local Qwen3-TTS

AivoRelay can use the official Apache-2.0
`Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice` model without sending synthesis text or
audio to a cloud provider. The model and isolated runtime are not bundled in
the app installer; installation is an explicit multi-gigabyte download.

```powershell
# Inspect installation state and the pinned model revision
AivoRelay.exe tts-local status
AivoRelay.exe tts-local status --json

# Explicitly download/install the managed runtime and official model
AivoRelay.exe tts-local install --yes

# Generate a real bilingual diagnostic file through the normal TTS pipeline
AivoRelay.exe tts-local test --output .\local-tts-test.mp3
AivoRelay.exe tts-local test --output .\local-tts-test.wav --voice Serena --language Russian

# Remove only the managed local runtime/model; TTS History remains intact
AivoRelay.exe tts-local delete --yes
```

Installation reuses the app's resumable Hugging Face download behavior,
verifies the pinned model files, and keeps Python, `uv`, PyTorch, and Qwen
isolated from the user's PATH and Python environments. CUDA is selected when a
compatible NVIDIA driver is detected; otherwise AivoRelay installs the CPU
profile and warns that synthesis may be slower than real time.

After installation, select **Qwen3-TTS (Local)** on the Text to Speech page.
Normal `--convert-file`, folder automation, History, and regeneration commands
then use the same preprocessing, semantic chunking, retry, resume, WAV, and
MP3/256 kb/s pipeline as cloud providers. CLI conversion never starts an
implicit model download: a missing/incomplete installation returns an
actionable error.

`tts-local test` does not change the saved provider or other TTS settings. Its
MP3 output uses the normal default of 256 kb/s.

See also [[CLI-SPEECH-TO-TEXT|CLI Speech-to-Text File Conversion]].

## TTS History CLI

TTS History is separate from transcription history and is divided into
independent `file` and `interactive` scopes. File History is the CLI default;
pass the global `--scope interactive` option for clipboard/selection/overlay
results. Existing records remain available even when new capture is disabled.

```powershell
AivoRelay.exe tts-history list
AivoRelay.exe tts-history list --limit 20
AivoRelay.exe tts-history list --group "source-group-id"
AivoRelay.exe tts-history show 42
AivoRelay.exe tts-history --scope interactive list
AivoRelay.exe tts-history --scope interactive show 7
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

Cloud regeneration makes a **new paid TTS API request**. Local Qwen and Windows
voice regeneration do not use API credits. The source result and every
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

Supported providers are `soniox`, `deepgram`, `openai`, `local-qwen`, and
`windows`. For Windows, `--voice` is the stable WinRT installed-voice ID, not
its display name; an empty saved voice selects the current Windows default.
Before synthesis, AivoRelay resolves that default to one concrete stable ID for
the whole operation and its resume checkpoint. History list/show output includes
the recorded language together with the provider, model, and voice identity.
Windows voices require no model download or API key. AivoRelay does not
redistribute them, and usage/licensing rights depend on the installed voice
and its provider terms.
Supported formats are `mp3` and `wav`; MP3 bitrates are `64`, `96`, `128`,
`192`, `256`, and `320` kb/s. Provider credentials, retry, chunking,
preprocessing, speed, and other behavior still come from the saved Text to
Speech settings. `local-qwen` requires a completed explicit `tts-local install`
but does not require `--yes` for History regeneration because no paid API call
is made.

Interactive and File History have independent opt-in, maximum-result, and
managed-audio limits. Manual file conversion, ordinary CLI conversion, and
folder automation enter File History; clipboard, selected-text, and overlay
playback enter Interactive History. Regeneration stays in its source scope.
After a successful save, AivoRelay removes only that scope's oldest managed
results until both of its limits are satisfied.

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
retained/saved prompt. Prompt instructions require OpenAI with a compatible
`gpt-4o-mini-tts` model. The official Qwen3-TTS 0.6B runtime does not apply
`instruct`, so local regeneration preserves but does not send saved prompts.

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
