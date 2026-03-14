# Upstream Sync Log

Small rolling log of upstream commits integrated into `main`.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Local SHA | Local Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-14 | 2026-03-14 | `85a8ed7` | feat: pass custom words as Whisper initial_prompt instead of post-correction (#1035) | `794d651` | Bias Whisper custom words via initial prompt | added Whisper biasing; kept fork post-correction path |
| 2026-03-14 | 2026-03-12 | `aebd432` | fix: overlay not showing on non-primary monitors (#969) | `bcbd97f` | Fix overlay placement on non-primary monitors | extended to shared overlay helpers/live preview geometry |
| 2026-03-14 | 2026-03-11 | `82297fa` | Add model loading failure notifications with i18n support (#997) | `7f99cb5` | Notify on model loading failures | reused existing `loading_failed`; added shared TS event type; en/ru only |
| 2026-03-14 | 2026-03-10 | `785c331` | Handle microphone init failure without aborting (#945) | `e890d40` | Surface recording start failures in UI | backend already in fork; ported missing `recording-error` UI path |
| 2026-03-14 | 2026-03-06 | `615b3c9` | feat: language-aware filler word removal (#971) | `871eb1e` | Port language-aware filler word removal | adapted to transcription language; bindings not regenerated |
| 2026-03-05 | 2026-03-01 | `17d34a9` | fix: upgrade tauri-plugin-updater to v2.10.0 to fix duplicate registry entries (#873) (#876) | `6164c50` | fix: upgrade tauri and updater to 2.10.x (from 17d34a9) | cherry-pick aborted (4-way conflicts); selective manifest intake; lockfiles untouched; diff saved |
| 2026-03-05 | 2026-03-01 | `eade87a` | upgrade to handy keys 0.2.2 (#926) | `3452e0b` | chore(deps): bump handy-keys to 0.2.2 | selective intake: Cargo.toml only; no upstream Cargo.lock/i18n |
| 2026-03-05 | 2026-03-01 | `f403cb1` | update transcribe-rs | `0ca85fa` | update transcribe-rs | Cargo.lock conflict resolved with ours; diff saved |
| 2026-03-04 | 2026-03-02 | `a6b5c32` | move to tauri dialog 2.6 | `ba650a3` | move to tauri dialog 2.6 | conflicts in Cargo.toml/Cargo.lock; lock=ours; diff saved |
| 2026-03-04 | 2026-02-25 | `f1516d9` | fix: auto-refresh model list when switching post-processing providers (#854) | `1a4bd4c` | fix: auto-refresh model list when switching post-processing providers (#854) | clean cherry-pick |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | 'local_sha' | local message | short issue note |`
