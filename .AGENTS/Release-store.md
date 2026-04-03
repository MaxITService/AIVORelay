Branch tags: #branch/microsoft-store

# Release Store

Branch-specific release rules for the Microsoft Store Edition.

## Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks.

- These files are fragile.
- Keep YAML indentation consistent.
- Avoid clever multi-line script formatting in YAML.
- If workflow YAML changed and the user asks for verification, validate it before commit.

## Version Bump

When asked to bump the app version:

1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. Stop before commit and ask the user to run the build/check flow on their side unless they explicitly asked for local verification.
5. If `src-tauri/Cargo.lock` changed because of the version bump, include it in the same commit.
6. Before final commit, ask whether a new Store release body draft is needed.
7. If yes, prepare a short user-facing draft for `.github/release-notes/microsoft-store.md`.
8. Commit with `chore: bump version to x.y.z`.
9. Create tag or push only when the user explicitly asks.

## Tags And Pushes

- Keep the app version numeric, for example `1.0.2`.
- Use tag `vx.y.z-store` only when the user explicitly asks for the Store tag.
- Push `Microsoft-store` and the Store tag only when the user explicitly asks.
