# Branch Propagation Log

Small rolling log of `main` commits propagated into non-`main` release branches.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- Use one row per branch propagation event.
- On new entry #11, remove the oldest row.
- After a successful propagation, mirror the same new row in both `main` and the target branch worktree copy of this file.
- Keep issue notes very short.

| Propagation Date | Target Branch | Main SHA | Main Message | Branch SHA | Branch Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-04-01 | `cuda-integration` | `838de043` | Add overlay icon and decap indicator customization | `580b7b4d` | Add overlay icon and decap indicator customization | clean cherry-pick |
| 2026-03-30 | `cuda-integration` | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `93883713` | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; kept local fork patches |
| 2026-03-29 | `cuda-integration` | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `fea50f23` | fix(transcription): harden idle unload and windows CI build path | build.yml auto-merge |
| 2026-03-29 | `cuda-integration` | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `8b080182` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `4725eca3` | fix(tray): show app version in tooltip | `f2fbfe68` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `f410d788` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `305c6878` | fix(settings): repair invalid portions on load | `4a983f07` | fix(settings): repair invalid portions on load | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `37e5a204` | fix(ci): shorten Windows cargo target dir for whisper build | `1c886375` | fix(ci): shorten Windows cargo target dir for whisper build | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `9cf5c37a` | fix(ui): keep dropdown menus above settings panels | `0aec707f` | fix(ui): keep dropdown menus above settings panels | clean cherry-pick |
| 2026-03-26 | `cuda-integration` | `d0db58e4` | chore(bindings): regenerate after gpu selector sync | `65b99aba` | chore(bindings): regenerate after gpu selector sync | includes 637f1e6a; kept CUDA-specific transcribe-rs dependency path |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
