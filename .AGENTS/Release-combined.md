Branch tags: #branch/codex-combined

# Combined Release Rules

Read this file only when preparing a release or version bump for `codex/combined`.

## Version Bump

When asked to bump the app version:

1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. Stop before commit and ask the user to run the relevant combined build/check flow on their side.
5. After user build is done, re-check git status.
6. If `src-tauri/Cargo.lock` changed due to the version bump, include it in the same commit.
7. Before final commit, ask whether a combined release body draft is needed.
8. Commit with `chore: bump version to x.y.z`.
9. After commit, ask whether to create tag and push now.

## Combined Release Packaging

- Confirm whether the intended release should include:
  - standard app + AVX2 sidecar
  - standard app + AVX2 sidecar + CUDA sidecar
- The normal local release-prep path for the full combined package is:

```powershell
pwsh -NoProfile -File .\build-local.ps1 -Cuda
```

- Prefer packaged release-build verification over dev-server testing for combined runtime switching.

## Tags And Pushes

- Tag `vx.y.z-combined` only when the user explicitly asks.
- Push `codex/combined` and the combined tag only when the user explicitly asks.

## Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks.

Rules:

1. Never edit workflow YAML without explicit user approval.
2. Never add new inputs or parameters to reusable workflows such as `.github/workflows/build.yml` unless the user explicitly asks for that contract change.
3. Inside YAML `script: |` blocks, every content line must stay indented consistently.
4. Avoid multi-line JavaScript template literals inside YAML `script: |` blocks; prefer plain strings with `\n` or simple concatenation.
5. If workflow YAML was edited, validate the file before committing when the user asks for verification.

## Release Body Drafting

- If the user wants a combined release body draft, build a short user-facing summary from branch-relevant commits only.
- Exclude internal-only items unless the user asks to include them.
- If the branch later needs a checked-in release body file, confirm with the user before adding one.
