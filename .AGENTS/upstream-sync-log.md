# Upstream Sync Log

Small rolling log of upstream commits integrated into `main`.

Audit note (2026-03-21):
- Current fetched `upstream/main` head checked locally: `58cda3f3`.
- Safe review cursor for the next `upstream -> main` intake: `58cda3f3`.
- The table below logs integrated upstream commits only; the review cursor may be newer because it also accounts for explicitly skipped commits.
- Re-triaged corridor up to `58cda3f3`: taken/logged `58cda3f3`, `e35f0a71`, `cb32d35b`, `0b3322fa`, `e1a484f7`, `5a3e6e33`, `2eeb2129`; already covered `095f4ac4`; skipped `8836d455`, `1a95c9c4`, `cd3ec3ab`, `c5ec92b3`, `e3c9f581`, `075a5887`, `012e0666`, `d33535cf`; treated `a3015026` as separate research / split adaptation, not a normal intake row.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Local SHA | Local Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-21 | 2026-03-21 | `58cda3f3` | fix: sha256 verification to prevent corrupt partial download loop (#1095) | `4d4b46db` | fix(models): verify downloads and clear corrupt partials | partial port; runtime fix kept, UI/state handling simplified |
| 2026-03-21 | 2026-03-21 | `e35f0a71` | improve history performance (#1107) | `f0eb727b` | perf(history): paginate history settings list | adapted for fork history fields; raw invoke/listen instead of bindings |
| 2026-03-21 | 2026-03-19 | `cb32d35b` | feat(audio): lazy stream close for bluetooth mic latency (#747) | `54696496` | feat(audio): add lazy mic stream close toggle | adapted; backend intent kept, Debug UI/bindings diverged |
| 2026-03-21 | 2026-03-19 | `0b3322fa` | feat(audio): use device default sample rate and always downsample (#1084) | `7c6b59f6` | feat(audio): prefer device default mic sample rate | well taken for mic path; loopback kept fork-specific |
| 2026-03-18 | 2026-03-18 | `e1a484f7` | ci: reduce PR check time from ~30 min to ~1 min (#1073) | `5dc33a2d` | Consolidate PR code quality checks | partial port only; merged lint+prettier with concurrency/path filters, skipped nix/playwright/test pieces |
| 2026-03-18 | 2026-03-18 | `5a3e6e33` | add extra recording buffer (#1089) | `ba2bba60` | Add local-only extra recording buffer | partial port only; kept remote Soniox/Deepgram buffer behavior unchanged |
| 2026-03-18 | 2026-03-18 | `2eeb2129` | upgrade path from old giga-am to new (#1088) | `25a03b17` | Port GigaAM v3 directory migration | adapted onto fork transcribe-rs 0.3.x state; added vocab resource + old-format migration |
| 2026-03-17 | 2026-03-17 | `d1da9354` | fix: auto-unload model after idle timeout to reduce memory (#1051) | `70163254` | Improve idle model unload behavior | adapted onto existing fork unload flow; kept fork events/settings structure |
| 2026-03-17 | 2026-03-16 | `cafc2b72` | experimental: pick between cpu/gpu acceleration + enable directml on windows (#1058) | `02ce4b07` | Port Canary models and accelerator settings | ported from Handy HEAD; adapted to fork commands/UI/settings |
| 2026-03-17 | 2026-03-16 | `f8bbcd79` | Migrate to transcribe-rs-0.3.1 and add Canary support (#1023) | `02ce4b07` | Port Canary models and accelerator settings | combined with accel intake in one local port commit |
| 2026-03-17 | 2026-03-16 | `f45ad97e` | ensure samples don't get dropped (#1043) | `3346d7d8` | ensure samples don't get dropped (#1043) | merged into fork recorder with loopback/callback support intact |
| 2026-03-14 | 2026-03-11 | `dfd445d` | Add Windows microphone permission onboarding (#991) | `f3d8c86` | Add Windows microphone permission onboarding | adapted onto fork onboarding; covers remote-only and local setups; frontend uses `invoke` |
| 2026-03-14 | 2026-03-06 | `e354c0a` | update dialog package.json | `c6dec17` | Pin dialog plugin to 2.6.x | current fork already on 2.6; lockfile intentionally untouched |
| 2026-03-14 | 2026-03-11 | `d6ed1f9` | ui: improve scrollbar UI with custom colors and rounded thumb (#983) | `a272fd6` | Refine themed scrollbar styling | adapted onto fork dark theme instead of replacing existing look |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | 'local_sha' | local message | short issue note |`
