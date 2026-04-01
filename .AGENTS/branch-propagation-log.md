# Branch Propagation Log

Small rolling log of `main` commits propagated into non-`main` release branches.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per branch propagation event.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.

| Propagation Date | Target Branch | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-04-01 | `codex/combined` | `838de043` | Add overlay icon and decap indicator customization | `e265f3e2` | Add overlay icon and decap indicator customization | code-notes merge |
| 2026-03-30 | `codex/combined` | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `c4790a64` | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; standard path restored upstream |
| 2026-03-29 | `codex/combined` | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `966e08c0` | fix(transcription): harden idle unload and windows CI build path | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `2f3fe939` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `4725eca3` | fix(tray): show app version in tooltip | `1bcda96c` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `4bcc0a45` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `305c6878` | fix(settings): repair invalid portions on load | `b598c953` | fix(settings): repair invalid portions on load | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `305c6878` | fix(settings): repair invalid portions on load | `4a983f07` | fix(settings): repair invalid portions on load | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `1c886375` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |
| 2026-03-26 | `Microsoft-store` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `a6974073` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
