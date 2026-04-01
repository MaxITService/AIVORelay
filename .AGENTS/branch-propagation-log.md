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
| 2026-04-01 | `cuda-integration` | `838de043` | Add overlay icon and decap indicator customization | `580b7b4d` | Add overlay icon and decap indicator customization | clean cherry-pick |
| 2026-04-01 | `Microsoft-store` | `838de043` | Add overlay icon and decap indicator customization | `d56d72de` | Add overlay icon and decap indicator customization | clean cherry-pick |
| 2026-03-30 | `codex/combined` | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `c4790a64` | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; standard path restored upstream |
| 2026-03-30 | `cuda-integration` | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `93883713` | build(transcription): upgrade transcribe-rs to 0.3.5 | manual port; kept local fork patches |
| 2026-03-30 | `Microsoft-store` | `3aaeda2b` | build(transcription): upgrade transcribe-rs to 0.3.5 | `9d608233` | build(transcription): upgrade transcribe-rs to 0.3.5 | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `966e08c0` | fix(transcription): harden idle unload and windows CI build path | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `fea50f23` | fix(transcription): harden idle unload and windows CI build path | build.yml auto-merge |
| 2026-03-29 | `Microsoft-store` | `9b99c39c` | fix(transcription): harden idle unload and windows CI build path | `70de5cdf` | fix(transcription): harden idle unload and windows CI build path | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `2f3fe939` | perf(audio): preload recorder during local model warmup | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
