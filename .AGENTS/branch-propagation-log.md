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
| 2026-03-29 | `codex/combined` | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `2f3fe939` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `8b080182` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `Microsoft-store` | `ecb1fbdb` | perf(audio): preload recorder during local model warmup | `9d8a8e48` | perf(audio): preload recorder during local model warmup | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `4725eca3` | fix(tray): show app version in tooltip | `1bcda96c` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `4725eca3` | fix(tray): show app version in tooltip | `f2fbfe68` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `Microsoft-store` | `4725eca3` | fix(tray): show app version in tooltip | `f58825a8` | fix(tray): show app version in tooltip | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `4bcc0a45` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `cuda-integration` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `f410d788` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `Microsoft-store` | `9a15c63b` | fix: redact stored secrets in settings debug logs | `86e2dd8b` | fix: redact stored secrets in settings debug logs | clean cherry-pick |
| 2026-03-29 | `codex/combined` | `305c6878` | fix(settings): repair invalid portions on load | `b598c953` | fix(settings): repair invalid portions on load | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | target-branch | 'main_sha' | main message | 'branch_sha' | branch message | short issue note |`
