# CUDA Release Runbook

Read this file only when preparing a release or version bump for `cuda-integration`.

## Version Bump

When asked to bump the app version:

1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. Stop before commit and ask the user to run the build/check flow on their side.
5. After user build is done, re-check git status.
6. If `src-tauri/Cargo.lock` changed due to the version bump, include it in the same commit.
7. Before final commit, ask whether a new CUDA release body draft is needed.
8. Commit with `chore: bump version to x.y.z`.
9. After commit, ask whether to create tag and push now.

## CUDA Tags And Release Files

- Tag `vx.y.z-cuda` only when the user explicitly asks.
- Push `cuda-integration` and the CUDA tag only when the user explicitly asks.
- Release body file: `.github/release-notes/cuda.md`

## CUDA Release Workflow

- Use the CUDA release workflow for this branch.
- The CUDA release workflow builds a portable zip and expects:
  - `MaxITService/AIVORelay-dep-transcribe-rs`
  - `MaxITService/AIVORelay-dep-whisper-rs`

## Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks.

Rules:

1. Never edit workflow YAML without explicit user approval.
2. Never add new inputs or parameters to reusable workflows such as `.github/workflows/build.yml` unless the user explicitly asks for that contract change.
3. Inside YAML `script: |` blocks, every content line must stay indented consistently.
4. Avoid multi-line JavaScript template literals inside YAML `script: |` blocks; prefer plain strings with `\n` or simple concatenation.
5. If workflow YAML was edited, validate the file before committing when the user asks for verification.

## Release Body Drafting

When preparing release text for user review:

1. Start only after user confirms that a new release body draft is needed.
2. Build a short, user-facing summary from commits between the previous CUDA tag and current release commit.
3. Exclude internal-only items unless the user asks to include them.
4. Keep/update the baseline static notice text from `.github/release-notes/cuda.md`.
5. Present the final draft in chat and ask explicitly: use as-is or apply user-edited text from chat.
