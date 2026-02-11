# AivoRelay Microsoft Store Build

This is the Microsoft Store version of AivoRelay.

## Differences from standard version

| Feature | Store Version | Standard Version |
|---------|--------------|------------------|
| Installation | Microsoft Store | MSI Installer |
| Updates | Automatic (via Store) | Built-in Updater |
| Environment | Sandboxed | Standard Application |
| CPU Instructions | AVX2 only (wider compatibility) | AVX-512 auto-detected (faster on supported CPUs) |

## AVX-512 Disablement (AVX2 Build)

The Microsoft Store version targets a wider range of CPUs by **disabling AVX-512** and forcing **AVX2**. This ensures the app works on older processors that don't support AVX-512.

### How it works

Two config files control this:

1. **`.cargo/config.toml`** (project root):
   ```toml
   [env]
   # Force non-native CPU tuning so whisper-rs-sys does not auto-pick AVX512
   # on the build machine. On x64 this resolves to AVX2 defaults in ggml.
   WHISPER_NATIVE = "OFF"
   ```
   This prevents the whisper build system from detecting and using AVX-512 instructions on the CI/build machine.

2. **`src-tauri/.cargo/config.toml`** (does NOT exist on `main`):
   ```toml
   [build]
   rustflags = ["-C", "target-feature=+avx2"]
   ```
   This explicitly enables AVX2 optimizations for the Rust compiler.

### ⚠️ Do NOT remove these files or merge main's `.cargo/config.toml` over them

If these configs are lost during a merge, the build will auto-detect AVX-512 on the GitHub Actions runner, producing a binary that **crashes on older CPUs**.

## Branch-Specific File Changes (Merge Checklist)

When merging `main` into `Microsoft-store`, verify these differences are preserved:

| File | MS-store difference | What to check |
|------|-------------------|---------------|
| `.cargo/config.toml` | `WHISPER_NATIVE = "OFF"` | Must have the `[env]` section |
| `src-tauri/.cargo/config.toml` | `rustflags = ["-C", "target-feature=+avx2"]` | Must exist (not on `main`) |
| `src-tauri/tauri.conf.json` | Title: "AivoRelay (Store Edition)", empty `endpoints: []` | No update endpoints |
| `src-tauri/src/tray.rs` | "Check for Updates" menu item removed | No `check_updates_i` item |
| `src/components/footer/Footer.tsx` | No `UpdateChecker`, version shows "(Microsoft Store Edition)" | Must not import UpdateChecker |
| `src/components/settings/about/AboutSettings.tsx` | Version shows "(Microsoft Store Edition)" | Check line with `v{version}` |
| `src/i18n/locales/en/translation.json` | `tray.checkUpdates` key removed | Must not have this key |
| All other locale `translation.json` | `tray.checkUpdates` key removed | Already removed |
| `.github/workflows/build.yml` | Removed (not needed for store) | Should not exist |

## Updates

The Microsoft Store version is updated automatically through the Microsoft Store. The built-in Tauri update system is disabled in this version.

## Troubleshooting

If you encounter issues specific to the Store version, please report them on GitHub.

## How to identify Store version:
- Window title: "AivoRelay (Store Edition)"
- Footer shows version with "(Microsoft Store Edition)" suffix, e.g., "v0.7.9 (Microsoft Store Edition)"

## Common Pitfalls (How to Break the Build)

Do **NOT** do these things, or the build will fail:

1.  **Setting `targets: ["msix"]` in `tauri.conf.json`**:
    Tauri 2 (in our current configuration) will report this as an invalid target. Please use `msi`. Microsoft Store packaging is handled through a separate process.

2.  **Mangling JSX in `Footer.tsx`**:
    When modifying the footer to hide the `UpdateChecker`, ensure all `<div>` tags are properly balanced. Incorrect JSX structure will cause TypeScript compilation errors.

3.  **Renaming the version in configuration files**:
    GitHub Actions require a strict SemVer format. Do not add suffixes like `-Store` to the version in `tauri.conf.json` or `package.json`. Keep the "Store Edition" suffix in the UI (React components) only to ensure the release workflow functions correctly.

## Release Process

To create a release for the Microsoft Store:

1.  Go to the **Actions** tab in your GitHub repository.
2.  Select the **Microsoft Store Release** workflow from the sidebar.
3.  Click **Run workflow** and select the `Microsoft-store` branch.
    - **Important**: The branch search is **case-sensitive**. Type `Mi` or `Microsoft` (with capital M), not `mi`.
4.  This will create a draft release with the tag `vX.X.X-store` and assets named `aivorelay-store_*.msi`.
5.  Check the draft release and publish it when ready.

### GitHub Actions: workflow_dispatch Visibility

For the "Run workflow" button to appear in the GitHub Actions UI:

1.  The workflow file (`.github/workflows/microsoft-store-release.yml`) **must exist in the default branch** (`main`).
2.  Once it's in `main`, you can select any other branch (like `Microsoft-store`) when running it.
3.  If you create a new workflow only in a feature branch, it won't show up in the UI until merged to `main`.

This specialized workflow ensures that:
- The tag is distinct from standard releases (`-store` suffix).
- The release title clearly indicates the **Microsoft Store Edition**.
- Artifacts use the `aivorelay-store` prefix to avoid confusion with standard binaries.
