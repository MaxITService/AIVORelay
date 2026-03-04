# Upstream Sync Log

Small rolling log of integrated upstream commits.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Local SHA | Local Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-04 | 2026-03-02 | `a6b5c32` | move to tauri dialog 2.6 | `ba650a3` | move to tauri dialog 2.6 | conflicts in Cargo.toml/Cargo.lock; lock=ours; diff saved |
| 2026-03-04 | 2026-02-25 | `f1516d9` | fix: auto-refresh model list when switching post-processing providers (#854) | `1a4bd4c` | fix: auto-refresh model list when switching post-processing providers (#854) | clean cherry-pick |
| 2026-03-01 | 2026-03-01 | `ff86122` | feat: add GigaAM v3 for Russian speech recognition (#913) | `feb6f48` | feat: add GigaAM v3 for Russian speech recognition (#913) | none |
| 2026-02-22 | 2026-02-19 | `e624a45` | toast if exists | `dde5458` | Sync upstream features: drain audio and custom words duplicate toast | bundled with `3c0fb95` |
| 2026-02-22 | 2026-02-19 | `3c0fb95` | drain audio (#838) | `dde5458` | Sync upstream features: drain audio and custom words duplicate toast | bundled commit |
| 2026-02-19 | 2026-02-19 | `f8ee7fc` | feat: add z.ai post-process provider (#849) | `c205ae9` | feat: add z.ai post-process provider (#849) | none |
| 2026-02-19 | 2026-02-19 | `f367353` | fix handy-keys not firing when in the ui (#856) | `4b1e674` | fix handy-keys not firing when in the ui (#856) | none |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | \'up_sha\' | upstream message | \'local_sha\' | local message | short issue note |`
