Branch tags: #branch/microsoft-store

# Branch Propagation Log

Small rolling log of `main` commits propagated into `Microsoft-store`.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per `Microsoft-store` propagation event.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.

| Propagation Date | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- |
| 2026-04-02 | `4d2750b5` | fix: accept upstream old CPU crash fix | `86b32543` | fix: accept upstream old CPU crash fix | clean cherry-pick |
| 2026-04-01 | `3431d1db` | feat(models): show model details and supported languages | `e5be2f43` | feat(models): show model details and supported languages | includes 1027135c + afd68f69 |
| 2026-04-01 | `838de043` | Add overlay icon and decap indicator customization | `d56d72de` | Add overlay icon and decap indicator customization | clean cherry-pick |
| 2026-03-30 | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `9d608233` | build(transcription): upgrade transcribe-rs to 0.3.5 | clean cherry-pick |
| 2026-03-29 | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `70de5cdf` | fix(transcription): harden idle unload and windows CI build path | clean cherry-pick |
| 2026-03-29 | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `9d8a8e48` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `4725eca3` | fix(tray): show app version in tooltip | `f58825a8` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `9a15c63b` | fix: redact stored secrets in settings debug logs | `86e2dd8b` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `305c6878` | fix(settings): repair invalid portions on load | `749ed4b4` | fix(settings): repair invalid portions on load | clean cherry-pick |
| 2026-03-26 | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `a6974073` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
