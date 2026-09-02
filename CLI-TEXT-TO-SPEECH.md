# CLI Text-to-Speech File Conversion

Use AivoRelay's saved **Text to Speech** configuration to create MP3 or WAV
audio from a text or Markdown document.

## Basic commands

```powershell
AivoRelay.exe --convert-file .\chapter.md --output .\chapter.mp3
AivoRelay.exe --convert-file .\notes.txt --output .\notes.wav
AivoRelay.exe --convert-file .\chapter.md --output .\windows.wav --tts-provider windows
AivoRelay.exe --convert-file .\chapter.md --output .\edge.mp3 --tts-provider edge --tts-voice en-US-AriaNeural
AivoRelay.exe --convert-file .\chapter.md --output .\murf.mp3 --tts-provider murf --tts-model falcon-2
AivoRelay.exe --convert-file .\chapter.md --output .\elevenlabs.mp3 --tts-provider elevenlabs --tts-model eleven_multilingual_v2
AivoRelay.exe --convert-file .\chapter.md --output .\cartesia.mp3 --tts-provider cartesia --tts-model sonic-3.5
AivoRelay.exe --convert-file .\chapter.md --output .\qwen.mp3 --tts-provider local-qwen --tts-voice Vivian
AivoRelay.exe --convert-file .\chapter.md --output .\kokoro.mp3 --tts-provider local-kokoro --tts-voice af_maple --tts-language English
```

`.md` and `.txt` are first-class inputs. Markdown is converted to readable
text before TTS-only preprocessing, semantic chunking, retry, and synthesis.
The output extension selects MP3 or WAV. If `--output` is omitted, the saved
TTS output format is used beside the input file.

The CLI starts from the active provider/model profile in **Text file to mp3**
in the sidebar, then applies any command-line overrides to an in-memory copy.
Interactive hotkey settings are independent. Each page automatically remembers
its synthesis settings per provider/model, while named synthesis presets can be
loaded on either page. LLM cleanup prompts and presets remain separate. CLI
overrides never rewrite saved settings. When TTS History capture is enabled,
every successful CLI conversion also receives a managed History copy. Failure
to create that secondary copy is reported without discarding the successfully
created output file.

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
| `--tts-provider` | `soniox`, `deepgram`, `openai`, `murf`, `elevenlabs`, `cartesia`, `openai-compatible` (custom OpenAI-compatible server), experimental `edge`, `local-qwen`, `local-kokoro`, or `windows` |
| `--tts-model` | Provider model ID; Deepgram accepts Aura (`aura-*`) on `/v1/speak` and Flux (`flux-*`) on `/v2/speak`, Murf accepts `falcon-2`/`gen2`, ElevenLabs accepts `eleven_flash_v2_5`/`eleven_v3`/`eleven_multilingual_v2`, and Cartesia accepts `sonic-3.5` |
| `--tts-voice` | Cloud voice ID, official Qwen/Kokoro speaker ID, or stable Windows voice ID; `default` selects the current Windows default |
| `--tts-language` | Murf locale; a two-letter ISO 639-1 hint for ElevenLabs Flash v2.5/Eleven v3; a two-letter Cartesia language code; Soniox language code; Qwen language name; or Kokoro `English`/`Chinese`. ElevenLabs Multilingual v2 infers language from text and rejects this flag |
| `--tts-speed` | Soniox `0.7–1.3`; Deepgram Aura `0.7–1.5`; Deepgram Flux `0.85–1.15` in `0.05` steps; OpenAI `0.25–4.0`; ElevenLabs Flash v2.5/Multilingual v2 `0.7–1.2`; Cartesia Sonic 3.5 `0.6–1.5`; Edge/Qwen/Kokoro/Windows `0.5–2.0`. Murf uses `--tts-murf-rate`; Eleven v3 does not accept numeric speed |
| `--tts-key-source` | `shared` or `separate`; Murf, ElevenLabs, and Cartesia accept only `separate`. Selects an already-stored credential and never exposes a secret on the command line |
| `--tts-base-url` | OpenAI-compatible provider only: custom API base URL for this conversion, e.g. `http://localhost:8000/v1` |
| `--tts-murf-rate` | Murf integer rate from `-50` to `50` |
| `--tts-murf-pitch` | Murf integer pitch from `-50` to `50` |
| `--tts-murf-variation` | Murf Gen2-only variation from `0` to `5` |
| `--tts-murf-style` | Murf voice/locale style, or `none` to clear it |
| `--tts-elevenlabs-stability` | ElevenLabs stability from `0` to `1` |
| `--tts-elevenlabs-similarity-boost` | Flash v2.5/Multilingual v2 similarity boost from `0` to `1`; unavailable for Eleven v3 |
| `--tts-elevenlabs-style` | ElevenLabs style exaggeration from `0` to `1` |
| `--tts-elevenlabs-speaker-boost` | Flash v2.5/Multilingual v2: `true` or `false`; unavailable for Eleven v3 |
| `--tts-elevenlabs-text-normalization` | `auto`, `on`, or `off` |
| `--tts-cartesia-emotion` | Cartesia emotion, or `none` to clear it |
| `--tts-cartesia-volume` | Cartesia Sonic 3.5 generation volume from `0.5` to `2.0` |
| `--tts-format` | `mp3` or `wav`; with `--output`, it must match the extension |
| `--tts-bitrate` | MP3 only: `64`, `96`, `128`, `192`, `256`, or `320` kb/s |
| `--tts-chunk-chars` | `50` through the selected provider's hard character limit |
| `--tts-retries` | `0–10` retries after the first attempt |
| `--tts-retry-delay-ms` | `100–30000` ms initial exponential retry delay |
| `--tts-chunk-pause-ms` | `0–5000` ms |
| `--tts-paragraph-pause-ms` | `0–10000` ms |
| `--tts-preprocessing` | `true` or `false` for saved TTS replacement rules |
| `--tts-replacements-file` | UTF-8 JSON replacement-rule array used instead of saved rules for this run |
| `--tts-llm-preprocessing` | `true` or `false` for TTS File Operations AI cleanup |
| `--tts-llm-prompt` | Saved prompt name from the File Operations cleanup collection |
| `--tts-llm-instructions` | Literal one-off cleanup system prompt |
| `--tts-llm-instructions-file` | UTF-8 cleanup prompt file; takes precedence over inline/named prompt |
| `--tts-llm-provider` | Provider ID shared with the LLM provider catalog, such as `openrouter`, `openai`, or `custom` |
| `--tts-llm-model` | Exact cleanup-model ID |
| `--tts-llm-key-source` | `shared` reuses the secure LLM Post Processing key; `separate` uses the secure TTS cleanup key |
| `--tts-llm-base-url` | OpenAI-compatible URL; accepted only by the effective `custom` provider |
| `--tts-llm-allow-insecure-http` | `true` or `false`; accepted only by `custom` and intended for trusted local endpoints |
| `--tts-llm-reasoning` | `true` or `false` for compatible-provider reasoning controls |
| `--tts-llm-reasoning-budget` | `1024–1000000`; requires reasoning to be enabled |
| `--tts-llm-chunk-chars` | `1000–50000` Unicode characters; splitting prefers paragraph/sentence boundaries |
| `--tts-llm-retries` | `0–10` retries after the first cleanup request |
| `--tts-llm-retry-delay-ms` | `100–30000` ms initial exponential cleanup retry delay |
| `--tts-llm-timeout-seconds` | `10–600` seconds per cleanup request |
| `--tts-disk-reserve-mb` | `0–1048576` MB minimum free-space reserve |
| `--tts-history` | `true` or `false` for this result's History capture |

The final `--json` object reports the effective provider, model, voice,
language, key source, speed, provider-specific controls, format/bitrate, chunk/retry/pause settings,
preprocessing rule count, disk reserve, and History request state.

Provider-specific options are strict. AivoRelay returns exit code `2` with an
actionable explanation instead of silently ignoring a flag or clamping its
value:

- Deepgram encodes voice and language in its model ID, so use `--tts-model`; AivoRelay sends `flux-*` models to `/v2/speak` and Aura models to `/v1/speak` automatically;
  `--tts-voice` and `--tts-language` are rejected.
- OpenAI does not expose a separate language option, so `--tts-language` is
  rejected.
- Murf accepts only `falcon-2` and `gen2`; generic `--tts-speed` is rejected in
  favor of its integer rate control, and variation is accepted only for Gen2.
- ElevenLabs accepts `eleven_flash_v2_5`, `eleven_v3`, and
  `eleven_multilingual_v2`. Its voice
  settings and text-normalization controls are rejected for other providers.
  Flash v2.5 is the documented low-latency choice. Multilingual v2 infers
  language from the text and does not accept `--tts-language`; Eleven v3 does
  not accept numeric speed, Similarity, or Speaker Boost controls.
- Cartesia currently uses the fixed `sonic-3.5` model. Its 4,000-character
  request cap is an AivoRelay safety limit because Cartesia does not publish a
  single transcript-character maximum for the bytes endpoint. Speed, volume,
  and emotion are sent through Cartesia's `generation_config` object.
- Experimental Edge-TTS uses the fixed `microsoft-edge-read-aloud` service
  model and derives language from its voice ID, so use `--tts-voice`; model,
  language, and key-source flags are rejected. AivoRelay's native Rust client
  sends text directly to Microsoft Edge's online Read Aloud service without
  Python, a separately installed helper, or an API key. The unofficial service
  can change without notice.
- The local Qwen runtime uses AivoRelay's pinned model, so `--tts-model` is
  rejected; use `--tts-voice` and `--tts-language`.
- The local Kokoro runtime also uses a pinned model, so `--tts-model` is
  rejected. English accepts `af_maple`, `af_sol`, and `bf_vale`; Chinese uses
  the documented `zf_*` and `zm_*` voices.
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

## Optional AI text cleanup

AI cleanup is separate from voice instructions and deterministic replacement
rules. It runs first, then the cleaned result passes through replacement rules,
semantic speech chunking, and synthesis. Interactive reading and File
Operations have independent named prompt collections; `--convert-file` uses
only the File Operations collection.

```powershell
# Use the saved File Operations provider/model/prompt
AivoRelay.exe --convert-file .\scan.md --output .\scan.mp3 `
  --tts-llm-preprocessing true

# Select a saved File Operations cleanup prompt
AivoRelay.exe --convert-file .\scan.md --output .\scan.mp3 `
  --tts-llm-prompt "Remove page numbers and layout artifacts"

# Temporary provider/model/prompt overrides; saved settings are unchanged
AivoRelay.exe --convert-file .\article.md --output .\short.mp3 `
  --tts-llm-provider openrouter --tts-llm-model "provider/model-id" `
  --tts-llm-key-source shared `
  --tts-llm-instructions-file .\shorten-for-listening.txt
```

Any cleanup-specific override implies `--tts-llm-preprocessing true`.
Combining another `--tts-llm-*` option with
`--tts-llm-preprocessing false` fails before a network or synthesis request.
Provider/model/key errors include the provider response text after secret-safe
sanitization. Authentication, quota/billing, and invalid-request errors are not
retried; transient network, rate-limit, and 5xx failures retry only the current
cleanup chunk.

Cleanup instruction precedence is:

1. `--tts-llm-instructions-file`
2. `--tts-llm-instructions`
3. `--tts-llm-prompt`
4. the selected File Operations cleanup prompt

Cleanup prompts are limited to 32,768 Unicode characters. The same Windows
command-line limits described below apply, so prefer an instructions file for
long prompts. API keys are never accepted as CLI arguments.

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
models that do not support instructions, experimental Edge-TTS, local
Qwen3-TTS 0.6B, local Kokoro 82M, and Windows installed voices.
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
voice instructions, effective provider-specific voice controls, speed, chunk plan, or pauses starts safely from zero;
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

## Optional local TTS engines

AivoRelay can use the official Apache-2.0
`Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice` model without sending synthesis text or
audio to a cloud provider. The model and isolated runtime are not bundled in
the app installer; installation is an explicit multi-gigabyte download.

```powershell
# Inspect installation state and the pinned model revision
AivoRelay.exe tts-local --engine qwen status
AivoRelay.exe tts-local --engine qwen status --json

# Explicitly download/install the managed runtime and official model
AivoRelay.exe tts-local --engine qwen install --yes

# Generate a real bilingual diagnostic file through the normal TTS pipeline
AivoRelay.exe tts-local --engine qwen test --output .\local-tts-test.mp3
AivoRelay.exe tts-local --engine qwen test --output .\local-tts-test.wav --voice Serena --language Russian

# Remove only the managed local runtime/model; TTS History remains intact
AivoRelay.exe tts-local --engine qwen delete --yes
```

Human-readable `status` output includes the exact source and author, pinned
revision, managed installation path, a conservative current disk-use estimate
(when files exist), installation allowance, web license, and local license
path. The estimate deduplicates large hard-linked model/runtime files while
conservatively counting small files. Running `install` without `--yes` prints
the same source, destination,
size, and license facts and asks the user to review the source and risks before
explicitly confirming. The settings UI requires two independent confirmations.

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

Kokoro 82M is the smaller, fast CPU-oriented option. AivoRelay pins the
official sherpa-onnx `kokoro-int8-multi-lang-v1_1` package (about 147 MB for
the model archive), verifies its SHA-256 before extraction, and keeps its
native runtime in a hidden persistent worker isolated from the app's
speech-to-text ONNX Runtime:

```powershell
AivoRelay.exe tts-local --engine kokoro status
AivoRelay.exe tts-local --engine kokoro install --yes
AivoRelay.exe tts-local --engine kokoro test --output .\kokoro-en.mp3 --voice af_maple --language English
AivoRelay.exe tts-local --engine kokoro test --output .\kokoro-zh.wav --voice zf_001 --language Chinese
AivoRelay.exe tts-local --engine kokoro delete --yes
```

Installation is explicit and resumable; normal conversion never downloads a
model implicitly. See the official
[Kokoro multi-language model documentation](https://k2-fsa.github.io/sherpa/onnx/tts/all/Chinese-English/kokoro-multi-lang-v1_1.html)
for its speaker catalog and samples. Kokoro and sherpa-onnx are Apache-2.0;
the installed notice bundle records the exact model, runtime, and licenses.

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
model, voice, output format, provider-specific synthesis controls, voice-prompt metadata, secret-free AI-cleanup
configuration/prompt identity, and whether the managed audio copy still exists.

Exporting copies retained audio and does **not** make an API request:

```powershell
AivoRelay.exe tts-history export 42 --output .\retained-copy.mp3
```

The output extension must match the retained MP3/WAV format. Existing files
are never overwritten.

### Regenerate a comparison variant

Cloud regeneration makes a **new paid TTS API request**. Local Qwen, Kokoro,
and Windows voice regeneration do not use API credits. The source result and every
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

# Re-run through a different AI cleanup model and saved scope-specific prompt
AivoRelay.exe tts-history regenerate 42 `
  --tts-llm-preprocessing true `
  --tts-llm-provider openrouter --tts-llm-model "provider/model-id" `
  --tts-llm-prompt "Create a concise listening edition" --yes
```

`--output` is optional for regeneration. Without it, AivoRelay stores only the
new managed History result; the default is MP3 at 256 kb/s. `--format wav`
selects managed WAV instead, and `--format mp3 --bitrate N` overrides the
managed MP3 bitrate. With `--output`, the output extension determines the
format and must match any explicit `--format`. Managed-only regeneration also
recovers verified chunks after a failed process and reports
`resumed_chunks`.

Supported providers are `soniox`, `deepgram`, `openai`, `murf`, `elevenlabs`,
`cartesia`, `openai-compatible`, experimental `edge`, `local-qwen`, `local-kokoro`, and `windows`. For Windows, `--voice` is the stable WinRT installed-voice ID, not
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
Speech settings. `local-qwen` and `local-kokoro` require a completed explicit
`tts-local --engine ... install`. Local speech regeneration does not itself
use credits, but `--yes` is still required when AI cleanup is enabled because
the cleanup provider may make a paid API request.

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
`gpt-4o-mini-tts` model. The official Qwen3-TTS 0.6B and Kokoro runtimes do
not apply instructions, so local regeneration preserves but does not send
saved prompts.

History regeneration accepts the same `--tts-llm-*` temporary overrides as
normal file conversion. File History uses the File Operations prompt
collection; Interactive History uses the interactive-reading collection.
Regeneration never opens the playback overlay, even for Interactive History.

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
