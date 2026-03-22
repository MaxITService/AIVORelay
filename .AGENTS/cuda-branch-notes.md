# CUDA Branch Notes

This note documents the current `cuda-integration` branch as a **content diff** against `main`.

Baseline used for this note:
- `main` content baseline: `5bcc9c6ba804186ca2842876b951c4d3fe2281fe`
- note date: `2026-03-22`
- this note describes the current files on disk; it is not a git-ancestry claim

Primary companion docs:
- [[CUDA]]
- [[.AGENTS/main-to-cuda-propagation-playbook|main-to-cuda-propagation-playbook.md]]

## How This Branch Works

- The branch is intended to stay as close to `main` as possible in normal app code.
- The branch-local layer is the CUDA/build/release/dependency wiring needed for NVIDIA-enabled Windows builds.
- Local CUDA builds use [[build-cuda.ps1]] instead of the default `main` flow.
- The local build path imports Visual Studio tools, configures LLVM/bindgen, sets CUDA 12.4, rewrites Cargo patch paths to local dependency forks, runs `bun install`, then runs `tauri build --no-bundle`.
- The dependency chain is intentionally local-first:
  - `C:\Code\AIVORelay-deps\AIVORelay-dep-transcribe-rs`
  - `C:\Code\AIVORelay-deps\AIVORelay-dep-whisper-rs`
  - `C:\Code\AIVORelay-deps\AIVORelay-dep-whisper-rs\sys`
- `src-tauri/Cargo.toml` points to `transcribe-rs 0.3.2` with `whisper-cpp` + `onnx`, and on Windows additionally enables `whisper-cuda` + `ort-cuda`.
- `src-tauri/Cargo.lock` is expected to resolve against the local fork graph, not against `main`'s crates.io/vendored `whisper-rs-sys` arrangement.
- `.cargo/config.toml` moves the target dir to `C:/aivorelay-cuda` and uses `lld-link` to keep Windows CUDA builds practical.

## Files Different From `main`

These are the current branch-local files relative to `main@5bcc9c6b`, including the new branch-only note file that does not exist on `main`.

| File | How it differs from `main` |
| --- | --- |
| `.AGENTS/Release.md` | Keeps CUDA-specific release rules: `vx.y.z-cuda`, CUDA workflow usage, and dependency repo expectations for CUDA releases. |
| `.AGENTS/branch-propagation-log.md` | Reset to the new CUDA baseline on `2026-03-22`; now records the branch as content-aligned to `main@5bcc9c6b` and is meant to grow forward from there. |
| `.AGENTS/branching-status.md` | CUDA cursor is documented from the new baseline (`5bcc9c6b`) instead of the older propagation point. |
| `.AGENTS/cuda-branch-notes.md` | New branch-only note: the authoritative file-by-file `cuda-integration -> main` manifest and dependency-model explanation. |
| `.AGENTS/code-notes.md` | Adds an explicit pointer that branch-vs-main CUDA notes live in this file, while `code-notes.md` itself remains fork-vs-upstream focused. |
| `.AGENTS/main-to-cuda-propagation-playbook.md` | Adds an explicit rule that after each propagation, `CUDA.md` and this note must be refreshed so the documented branch-local layer stays accurate. |
| `.AGENTS/upstream-sync-log.md` | The CUDA worktree copy carries branch-local sync audit text; no runtime effect. |
| `.cargo/config.toml` | Adds `target-dir = "C:/aivorelay-cuda"` and `linker = "lld-link"` for Windows CUDA builds. |
| `.github/workflows/cuda-release.yml` | Expands the CUDA release workflow into a Windows CUDA release pipeline: local dependency repo checkout, recursive `whisper.cpp` submodule checkout, certificate import, LLVM/Ninja/Vulkan/CUDA setup, Tauri build, MSI/portable zip upload, and certificate helper asset upload. |
| `AGENTS.md` | Documents CUDA branch references/worktree expectations and links to this CUDA branch note. |
| `CUDA.md` | Human-readable CUDA branch overview: dependency model, local build path, runtime assumptions, and a pointer to this exact file manifest. |
| `README.md` | Keeps the CUDA Edition release mention and now points readers to `CUDA.md` for branch-specific build/dependency notes. |
| `build-cuda.ps1` | Adds the local CUDA build/dev entrypoint: VS env import, bindgen env setup, CUDA path setup, dependency-root validation, Cargo patch path rewrite, log capture, and `tauri build/dev` invocation. |
| `src-tauri/Cargo.toml` | Uses the CUDA/local-fork dependency model instead of `main`'s default: `transcribe-rs 0.3.2`, Windows `whisper-cuda` + `ort-cuda`, and local `[patch.crates-io]` entries for `transcribe-rs`, `whisper-rs`, and `whisper-rs-sys`. |
| `src-tauri/Cargo.lock` | Resolves the branch against the local CUDA dependency graph (`transcribe-rs 0.3.2`, local `whisper-rs 0.13.2`, local `whisper-rs-sys 0.11.2`) rather than the `main` branch lock layout. |

## Practical Rule

When syncing `main -> cuda-integration`, the target state is:
- ordinary app code follows `main`
- only the files listed above should remain branch-local unless the user explicitly approves a wider CUDA divergence
