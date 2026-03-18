# Upstream Sync Log

Small rolling log of upstream commits integrated into `main`.

Rules:
- Keep newest entries first.
- Keep only last 10 entries.
- On new entry #11, remove the oldest row.
- Keep issue notes very short.

| Merge Date | Upstream Date | Upstream SHA | Upstream Message | Local SHA | Local Message | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-03-18 | 2026-03-18 | `2eeb2129` | upgrade path from old giga-am to new (#1088) | `25a03b17` | Port GigaAM v3 directory migration | adapted onto fork transcribe-rs 0.3.x state; added vocab resource + old-format migration |
| 2026-03-17 | 2026-03-17 | `d1da9354` | fix: auto-unload model after idle timeout to reduce memory (#1051) | `70163254` | Improve idle model unload behavior | adapted onto existing fork unload flow; kept fork events/settings structure |
| 2026-03-17 | 2026-03-16 | `cafc2b72` | experimental: pick between cpu/gpu acceleration + enable directml on windows (#1058) | `02ce4b07` | Port Canary models and accelerator settings | ported from Handy HEAD; adapted to fork commands/UI/settings |
| 2026-03-17 | 2026-03-16 | `f8bbcd79` | Migrate to transcribe-rs-0.3.1 and add Canary support (#1023) | `02ce4b07` | Port Canary models and accelerator settings | combined with accel intake in one local port commit |
| 2026-03-17 | 2026-03-16 | `f45ad97e` | ensure samples don't get dropped (#1043) | `3346d7d8` | ensure samples don't get dropped (#1043) | merged into fork recorder with loopback/callback support intact |
| 2026-03-14 | 2026-03-11 | `dfd445d` | Add Windows microphone permission onboarding (#991) | `f3d8c86` | Add Windows microphone permission onboarding | adapted onto fork onboarding; covers remote-only and local setups; frontend uses `invoke` |
| 2026-03-14 | 2026-03-06 | `e354c0a` | update dialog package.json | `c6dec17` | Pin dialog plugin to 2.6.x | current fork already on 2.6; lockfile intentionally untouched |
| 2026-03-14 | 2026-03-11 | `d6ed1f9` | ui: improve scrollbar UI with custom colors and rounded thumb (#983) | `a272fd6` | Refine themed scrollbar styling | adapted onto fork dark theme instead of replacing existing look |
| 2026-03-14 | 2026-03-14 | `e3a040c` | Remove `step` prop from VolumeSlider component (#944) | `abc12c7` | Simplify volume slider props | current slider already handles default stepping |
| 2026-03-14 | 2026-03-14 | `79f28f5` | remove usage of dangerouslySetInnerHTML | `f673a80` | Remove unsafe HTML in post-processing settings | used `Trans` for prompt tip; other fork HTML uses remain |

Entry template:

`| YYYY-MM-DD | YYYY-MM-DD | 'upstream_sha' | upstream message | 'local_sha' | local message | short issue note |`
