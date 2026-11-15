Branch tags: #branch/release-microsoft-store

# Microsoft Store Branch Contract

Canonical inventory of intentional differences from `main`. If code and this file disagree, verify the code and update this file in the same change.

## Distribution Differences

| Area | Microsoft Store Edition | Standard `main` edition |
| --- | --- | --- |
| Release identity | `aivorelay-store_*` files in the shared `vX.Y.Z` draft, built by `main`'s `release.yml` from this branch's head | Draft release tagged `vX.Y.Z` |
| Binary signing in GitHub Actions | `sign-binaries: false`; Microsoft Store ingestion handles final signing | Enabled by the standard release workflow |
| Project certificate and installation script | Must not be bundled in MSI/ZIP or attached to the GitHub release | Included as release attachments |
| Detached signature files | Must not be attached to the GitHub release | Updater signature files are generated |
| In-app updater artifacts | `createUpdaterArtifacts: false` | Enabled |
| In-app updater endpoints | Empty | GitHub release endpoint configured |
| Crash recovery screen | Reload window and open logs; no emergency self-update action | Also offers the emergency update checker |
| CPU baseline | Rust code is built with `+avx2`; ggml picks its CPU variant at runtime | Rust code targets baseline x86-64; ggml picks its CPU variant at runtime |

Before completing a Store release, check that GitHub release assets contain no project certificate, certificate-installation script, or detached signature files. Remove such attachments added to a Store draft by mistake; do not rebuild packages to upload them. Binary signature inspection is not required and an existing binary signature is not a release blocker.

Implementation anchors:

- Release behavior: `main`'s `.github/workflows/release.yml` and `build.yml`; this branch has no release workflow
- Updater/package behavior: `src-tauri/tauri.conf.json`
- CPU baseline: `src-tauri/.cargo/config.toml`

## Shared Application Source

From 1.0.54, `src/`, `src-tauri/src/`, and `src-tauri/build.rs` are identical to main. The branch-local `src-tauri/app-edition` contains `store`; main contains `standard`. The marker is compiled into the binary and never depends on the working directory or Git branch at runtime.

Store MSI/ZIP and MSIX all disable self-update and adjacent portable data storage through `distribution.rs`. Installer/ZIP detection is independent of edition. The Store build guard rejects nonempty updater endpoints or enabled updater artifacts. Keep AVX2, signing, packaging, lockfiles and agent documentation branch-local.

## Updater Boundary

- Microsoft Store delivery must not depend on AivoRelay self-update endpoints or updater artifacts.
- The shared `UpdateChecker` component and Tauri updater plugin still exist in this branch. Do not claim that the updater UI or plugin was completely removed.
- Portable updater helpers and translations are shared source. They are unreachable in Store because the edition locks update checks before any request or updater UI can run.
- Updater-only changes from `main` require manual review; do not propagate them automatically.
- The common crash recovery screen mounts the updater only when the backend explicitly reports update checks unlocked. Store remains limited to recovery/log actions; loading or query failures also keep the updater unmounted.
- The Store lockfiles currently retain updater `2.10.0` while `main` uses `2.10.1`. This is reviewed version drift, not a permanent API contract.

## CPU And Architecture Boundary

- The Store package requires an AVX2-capable x64 CPU. It is not a compatibility fallback for pre-AVX2 processors.
- Only `x86_64-pc-windows-msvc` Store files are published.
- ARM64 paths in reusable build/test workflows do not mean that an ARM64 Store package is published.
- The x64 workflow audits the portable ZIP and MSI for the transcribe.cpp core DLL, ggml backend modules, and required VC/OpenMP runtimes.

## Frontend Diagnostics

- Shared render recovery, uncaught-error logging, overlay boundaries, and their logging permissions follow `main`, with the crash-screen updater exclusion above.
- Hidden frontend sourcemaps are written to `dist-sourcemaps/`, outside the packaged `dist`, and uploaded as the separate `aivorelay-store-<target>-sourcemaps` CI artifact with 90-day retention. They are not GitHub release attachments or updater artifacts.

## Generated And Shared Files

- Never cherry-pick `src-tauri/Cargo.lock` from `main`; regenerate it from the Store manifest and verify it with `cargo metadata --locked`.
- Shared CLI behavior and its two CLI guides should normally match `main`.
- Review README, testing, help, and contribution documents manually after code propagation.
- Keep Store agent, release, configuration, updater, workflow, and CPU-baseline documentation branch-local.
