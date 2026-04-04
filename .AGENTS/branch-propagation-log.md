# Branch Propagation Log
Branch tags: #branch/main #branch/microsoft-store #branch/cuda-integration #branch/codex-combined

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
| 2026-04-04 | `Microsoft-store` | `5752185f` | change default model unload timeout to 15 minutes | `f7044b6b` | change default model unload timeout to 15 minutes | includes 09c0b163 + 943cd525 |
| 2026-04-04 | `cuda-integration` | `5752185f` | change default model unload timeout to 15 minutes | `b1b1a20a` | change default model unload timeout to 15 minutes | includes 09c0b163 + 943cd525 |
| 2026-04-04 | `codex/combined` | `5752185f` | change default model unload timeout to 15 minutes | `972a80fb` | change default model unload timeout to 15 minutes | includes 09c0b163 + 943cd525 |
| 2026-04-02 | `codex/combined` | `4d2750b5` | fix: accept upstream old CPU crash fix | `4ccf3ab7` | fix: accept upstream old CPU crash fix | clean cherry-pick |
| 2026-04-02 | `cuda-integration` | `4d2750b5` | fix: accept upstream old CPU crash fix | `4d1b6a19` | fix: accept upstream old CPU crash fix | build.yml auto-merge |
| 2026-04-02 | `Microsoft-store` | `4d2750b5` | fix: accept upstream old CPU crash fix | `86b32543` | fix: accept upstream old CPU crash fix | clean cherry-pick |
| 2026-04-01 | `codex/combined` | `3431d1db` | feat(models): show model details and supported languages | `0e6d14c9` | feat(models): show model details and supported languages | includes 1027135c + afd68f69; kept accel wiring |
| 2026-04-01 | `Microsoft-store` | `3431d1db` | feat(models): show model details and supported languages | `e5be2f43` | feat(models): show model details and supported languages | includes 1027135c + afd68f69 |
| 2026-04-01 | `cuda-integration` | `1027135c` | feat(transcription): port upstream Cohere support | `d97d5cc3` | feat(transcription): port upstream Cohere support | manual port; updated CUDA transcribe-rs fork |
| 2026-04-01 | `codex/combined` | `838de043` | Add overlay icon and decap indicator customization | `e265f3e2` | Add overlay icon and decap indicator customization | code-notes merge |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
