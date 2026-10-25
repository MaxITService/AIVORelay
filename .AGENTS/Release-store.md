Branch tags: #branch/release-microsoft-store

# Release Store

Branch-specific release rules for the Microsoft Store Edition.

## Release Contract

- No own release workflow or tag: `release.yml` on `main` builds this branch's pushed head (`x86_64-pc-windows-msvc` only, unsigned) into the shared `vX.Y.Z` draft as `aivorelay-store_X.Y.Z_x64_en-US.msi` and `aivorelay-store_X.Y.Z_x64_portable.zip`, and fails unless both versions match.
- Follow [[.AGENTS/store-branch-notes#Distribution Differences|the canonical signing, certificate, and release-asset rules]], including the final attachment check.
- Keep in-app updater artifacts disabled and updater endpoints empty.

## Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks.

- These files are fragile.
- Keep YAML indentation consistent.
- Avoid clever multi-line script formatting in YAML.
- If workflow YAML changed and the user asks for verification, validate it before commit.

## Fast Draft Release Path

If the user explicitly asks for the full Microsoft Store draft release flow, treat that as approval to bump the version, push this branch, and run `Release` on `main` without re-asking each step.
Keep the GitHub release as a draft unless the user explicitly asks to publish, and stop only for ambiguity, unusual risk, or a real conflict.

## Version Bump

When asked to bump the app version:

1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. Stop before commit and ask the user to run the build/check flow on their side unless they explicitly asked for local verification.
5. Regenerate the Store `src-tauri/Cargo.lock`; never copy the lockfile from `main`. Include the resulting version change in the same commit.
6. Commit with `chore: bump version to x.y.z`; release notes live in `main`'s `.github/release-notes/main.md`.
7. Push only when the user explicitly asks, before `Release` runs on `main`.

## Tags And Pushes

- Keep the app version numeric, for example `1.0.2`.
- Do not create Store tags or Store-only releases.
- Push `release/microsoft-store` only when the user explicitly asks.
