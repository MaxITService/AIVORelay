Branch tags: #branch/codex-combined

# Combined Branch Notes

This note documents the current `codex/combined` branch as a branch-local layer on top of `main`.

Baseline used for this note:

- latest propagated `main` cursor: `4d2750b5`
- note date: `2026-04-03`
- this note describes the current files on disk; it is not a git-ancestry claim

Primary companion docs:

- [[.AGENTS/build-combined|build-combined.md]]
- [[.AGENTS/Release-combined|Release-combined.md]]
- [[.AGENTS/branch-log-maintenance|branch-log-maintenance.md]]

## Branch Intent

- Ship one combined Windows distribution folder containing `AivoRelay.exe`, `aivorelay-avx2.exe`, and optionally `aivorelay-cuda.exe` when the CUDA sidecar is included for the requested build.
- Keep one shared application data/profile location so all variants use the same settings, logs, models, and history.
- Use one shared codebase; runtime logic detects which executable variant is currently running.
- If a better variant is available, the branch may prompt for restart or runtime switching instead of splitting builds into unrelated branches.
- Autostart and runtime switching must stay aligned with the selected executable variant.
- Prefer packaged release-build testing over dev-server testing for restart and handoff behavior.

## How This Branch Works

- The branch is intended to stay close to `main` in ordinary app code.
- The branch-local layer is combined packaging plus runtime variant detection.
- The standard executable uses the normal app build.
- The AVX2 and CUDA sidecars are thin launchers that run the shared app lib with variant-specific runtime identity.
- Local build flows prepare sidecars before packaging so Tauri can bundle them as `externalBin`.
- Runtime code reads the current executable name or `AIVORELAY_VARIANT` to identify the active variant and gate behavior such as self-update support.

## Current Branch-Local Files Relative To `main`

| File | How it differs from `main` |
| --- | --- |
| `AGENTS.md` | Short combined branch entry file that points non-branch-specific program work back to `main` docs and keeps only combined-local documentation links here. |
| `.AGENTS/MOC.md` | Small branch-local map of combined docs that remain in this worktree. |
| `.AGENTS/build-combined.md` | Branch-local build, sidecar packaging, bindings, and verification rules for combined work. |
| `.AGENTS/Release-combined.md` | Combined-specific release rules, packaging choices, and tag guidance. |
| `.AGENTS/combined-branch-notes.md` | This branch-only note describing current combined behavior and file-level divergence. |
| `.AGENTS/branch-log-maintenance.md` | Short local note for maintaining the combined branch log and cursor after successful `main -> codex/combined` syncs. |
| `.AGENTS/branch-propagation-log.md` | Records each propagated `main` commit reflected in this branch. |
| `.AGENTS/branching-status.md` | Combined cursor quick reference for the current sync point. |
| `build-local.ps1` | Branch-local local build path: short target dirs, sidecar preparation, variant-aware Tauri config overrides, and local artifact preservation. |
| `build-unsigned.js` | Release-build helper that prepares sidecars, injects `externalBin`, disables updater artifacts for unsigned builds, and preserves local artifacts. |
| `scripts/prepare-avx2-sidecar.js` | Builds and copies the AVX2 sidecar executable into `src-tauri/binaries`. |
| `scripts/prepare-cuda-sidecar.js` | Builds and copies the CUDA sidecar executable into `src-tauri/binaries`. |
| `src-tauri/.cargo/config.toml` | Uses a short target dir for branch-local Windows builds. |
| `src-tauri/cmake/force_ggml_avx2.cmake` | Forces ggml AVX2 flags for the AVX2 sidecar build. |
| `src-tauri/sidecars/aivorelay-avx2/*` | Thin AVX2 launcher package that runs the shared app lib as `aivorelay-avx2.exe`. |
| `src-tauri/sidecars/aivorelay-cuda/*` | Thin CUDA launcher package that runs the shared app lib as `aivorelay-cuda.exe`. |
| `src-tauri/Cargo.toml` | Adds sidecar workspace members and variant-specific acceleration features used by the combined packaging layer. |
| `src-tauri/tauri.conf.json` | Keeps `externalBin` wiring for branch-local sidecar packaging. |
| `src-tauri/src/runtime_info.rs` | Reports current executable variant and whether self-update is supported for that runtime. |
| `src/hooks/useAppRuntimeInfo.ts` | Frontend hook for runtime variant information from the backend. |

## Practical Rule

When syncing `main -> codex/combined`, the target state is:

- ordinary app code follows `main`
- only the combined packaging, runtime-variant, and local branch docs layer should remain branch-local unless the user explicitly approves wider divergence
