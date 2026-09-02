Branch tags: #branch/release-microsoft-store

# Microsoft Store Branch Contract

Canonical inventory of intentional differences from `main`. If code and this file disagree, verify the code and update this file in the same change.

## Distribution Differences

| Area | Microsoft Store Edition | Standard `main` edition |
| --- | --- | --- |
| Release identity | Draft pre-release tagged `vX.Y.Z-store` | Draft release tagged `vX.Y.Z` |
| Release target | The exact Store workflow commit | Normal release tag behavior |
| Binary signing in GitHub Actions | Disabled; Microsoft Store ingestion handles final signing | Enabled by the standard release workflow |
| In-app updater artifacts | `createUpdaterArtifacts: false` | Enabled |
| In-app updater endpoints | Empty | GitHub release endpoint configured |
| CPU baseline | Rust and ggml are forced to AVX2; AVX512/AMX is disabled | The x64 release workflow disables AVX/AVX2/FMA/F16C for older-CPU compatibility |

Implementation anchors:

- Release behavior: `.github/workflows/microsoft-store-release.yml` and `.github/workflows/build.yml`
- Updater/package behavior: `src-tauri/tauri.conf.json`
- CPU baseline: `src-tauri/.cargo/config.toml` and `src-tauri/cmake/force_ggml_avx2.cmake`

## Updater Boundary

- Microsoft Store delivery must not depend on AivoRelay self-update endpoints or updater artifacts.
- The shared `UpdateChecker` component and Tauri updater plugin still exist in this branch. Do not claim that the updater UI or plugin was completely removed.
- Store intentionally omits the portable-install fallback command, `portableInstaller.*`, and its translation strings.
- Updater-only changes from `main` require manual review; do not propagate them automatically.
- The Store lockfiles currently retain updater `2.10.0` while `main` uses `2.10.1`. This is reviewed version drift, not a permanent API contract.

## CPU And Architecture Boundary

- The Store package requires an AVX2-capable x64 CPU. It is not a compatibility fallback for pre-AVX2 processors.
- AVX512 and AMX must remain disabled for the distributed Store binary.
- The Microsoft Store release workflow currently publishes only `x86_64-pc-windows-msvc`.
- ARM64 paths in reusable build/test workflows do not mean that an ARM64 Store package is published.
- The x64 workflow audits the portable ZIP and MSI for the transcribe.cpp core DLL, ggml backend modules, and required VC/OpenMP runtimes.

## Generated And Shared Files

- Never cherry-pick `src-tauri/Cargo.lock` from `main`; regenerate it from the Store manifest and verify it with `cargo metadata --locked`.
- Shared CLI behavior and its two CLI guides should normally match `main`.
- Review README, testing, help, and contribution documents manually after code propagation.
- Keep Store agent, release, configuration, updater, workflow, and CPU-baseline documentation branch-local.
