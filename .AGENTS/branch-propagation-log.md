Branch tags: #branch/codex-combined

# Branch Propagation Log

Small rolling log of `main` commits propagated into `codex/combined`.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per `codex/combined` propagation event.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.

| Propagation Date | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- |
| 2026-04-04 | `5752185f` | change default model unload timeout to 15 minutes | `972a80fb` | change default model unload timeout to 15 minutes | includes 09c0b163 + 943cd525 |
| 2026-04-02 | `4d2750b5` | fix: accept upstream old CPU crash fix | `4ccf3ab7` | fix: accept upstream old CPU crash fix | clean cherry-pick |
| 2026-04-01 | `3431d1db` | feat(models): show model details and supported languages | `0e6d14c9` | feat(models): show model details and supported languages | includes 1027135c + afd68f69; kept accel wiring |
| 2026-04-01 | `838de043` | Add overlay icon and decap indicator customization | `e265f3e2` | Add overlay icon and decap indicator customization | code-notes merge |
| 2026-03-30 | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `c4790a64` | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; standard path restored upstream |
| 2026-03-29 | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `966e08c0` | fix(transcription): harden idle unload and windows CI build path | clean cherry-pick |
| 2026-03-29 | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `2f3fe939` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `4725eca3` | fix(tray): show app version in tooltip | `1bcda96c` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `9a15c63b` | fix: redact stored secrets in settings debug logs | `4bcc0a45` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `305c6878` | fix(settings): repair invalid portions on load | `b598c953` | fix(settings): repair invalid portions on load | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
