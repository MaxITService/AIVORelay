# Release Rules

## GitHub Actions / Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks for it.

These files are fragile. A broken workflow file can make GitHub Actions stop showing or running the expected workflow cleanly.

Rules:
1. Never edit workflow YAML without explicit user approval.
2. Never add new inputs or parameters to reusable workflows such as `.github/workflows/build.yml` unless the user explicitly asks for that contract change.
3. Inside YAML `script: |` blocks, every content line must stay indented consistently.
4. Avoid multi-line JavaScript template literals inside YAML `script: |` blocks; prefer plain strings with `\n` or simple concatenation.
5. If workflow YAML was edited, validate the file before committing when the user asks for verification.

## Version Bump

When asked to bump the app version:
1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. Do not run build or verification commands unless the user explicitly asks. This repo expects build verification to be user-driven.
5. Commit with `chore: bump version to x.y.z`.

## Tags And Branches

Use the same numeric app version on both release branches.

For `main`:
1. Tag `vx.y.z`.
2. Push `main` and the tag when the user explicitly asks to push.

For `Microsoft-store`:
1. Keep the app version numeric, for example `0.9.1`.
2. Use the Microsoft Store release workflow and store-specific release naming.
3. Tag `vx.y.z-store` only when the user explicitly asks for the store tag.
