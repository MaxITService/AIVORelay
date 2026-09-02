Branch tags: #branch/release-microsoft-store

# Release Store

Branch-specific release rules for the Microsoft Store Edition.

## Release Contract

- The Store workflow currently builds only `x86_64-pc-windows-msvc`.
- Create tag `vX.Y.Z-store` at the exact Store workflow commit.
- Create the GitHub release as a draft pre-release.
- Keep normal GitHub Actions binary signing disabled; Microsoft Store ingestion performs final signing.
- Keep in-app updater artifacts disabled and updater endpoints empty.
- Upload Store assets to the Store tag, never the plain `vX.Y.Z` tag.

## Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks.

- These files are fragile.
- Keep YAML indentation consistent.
- Avoid clever multi-line script formatting in YAML.
- If workflow YAML changed and the user asks for verification, validate it before commit.

## Fast Draft Release Path

If the user explicitly asks for the full Microsoft Store draft release flow, treat that as approval to bump the version, prepare the Store release notes, push the branch, and run the Store release workflow without re-asking each step.
Keep the GitHub release as a draft pre-release unless the user explicitly asks to publish, and stop only for ambiguity, unusual risk, or a real conflict.

## Version Bump

When asked to bump the app version:

1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. Stop before commit and ask the user to run the build/check flow on their side unless they explicitly asked for local verification.
5. Regenerate the Store `src-tauri/Cargo.lock`; never copy the lockfile from `main`. Include the resulting version change in the same commit.
6. Before final commit, ask whether a new Store release body draft is needed.
7. If yes, prepare a short user-facing draft for `.github/release-notes/microsoft-store.md`.
8. Commit with `chore: bump version to x.y.z`.
9. Create tag or push only when the user explicitly asks.
10. Use the Store-specific workflow and verify its draft pre-release targets `vx.y.z-store` and the exact Store commit.

## Tags And Pushes

- Keep the app version numeric, for example `1.0.2`.
- Use tag `vx.y.z-store` only when the user explicitly asks for the Store tag.
- Mark every GitHub release from `release/microsoft-store` as a pre-release.
- Push `release/microsoft-store` and the Store tag only when the user explicitly asks.
