# Release Rules
Branch tags: #branch/main #branch/microsoft-store #branch/cuda-integration #branch/codex-combined

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
4. Stop before commit and ask the user to run the build/check flow on their side (user-driven verification).
5. After user build is done, re-check git status. If `src-tauri/Cargo.lock` changed due version bump, include it in the same version-bump commit.
6. Do not run build or verification commands unless the user explicitly asks. This repo expects build verification to be user-driven.
7. Before final commit, ask the user whether a new release body draft is needed for the branch-specific Markdown file used by GitHub Actions.
8. If user says yes, prepare a short release body draft:
   - detect previous release tag for the branch (`vx.y.z` for `main`, `vx.y.z-store` for `Microsoft-store`, `vx.y.z-cuda` for `cuda-integration`);
   - review commits between previous tag and new version commit;
   - include only end-user facing changes in short bullets;
   - keep the standard static notice text that is normally used in release body.
9. Show the draft in chat and explicitly ask: use as-is, or user will provide edits in chat.
10. If the user approves the draft, write it into the matching checked-in file before commit:
    - `main`: `.github/release-notes/main.md`
    - `Microsoft-store`: `.github/release-notes/microsoft-store.md`
    - `cuda-integration`: `.github/release-notes/cuda.md`
11. Commit with `chore: bump version to x.y.z`.
12. After commit, ask whether to create tag and push now. 

## Tags And Branches

Use the same numeric app version on both release branches.

For `main`:
1. Tag `vx.y.z` only when the user explicitly asks.
2. Push `main` and the tag only when the user explicitly asks to push.

For `Microsoft-store`: if asked, prepare this branch too. If not asked, skip it entirely.
1. Keep the app version numeric, for example `0.9.1`. Same number is released in both branches, but only by user's apporval.
2. Use the Microsoft Store release workflow and store-specific release naming.
3. Tag `vx.y.z-store` only when the user explicitly asks for the store tag.
4. Push `Microsoft-store` and the store tag only when the user explicitly asks.

For `cuda-integration`: if asked, prepare this branch too. If not asked, skip it entirely.
1. Keep the app version numeric, for example `0.9.1`. Same number may be reused, but only by user's approval.
2. Use the CUDA release workflow and CUDA-specific release naming.
3. Tag `vx.y.z-cuda` only when the user explicitly asks for the CUDA tag.
4. Push `cuda-integration` and the CUDA tag only when the user explicitly asks.
5. The CUDA release workflow builds a portable zip and expects the dependency repos `MaxITService/AIVORelay-dep-transcribe-rs` and `MaxITService/AIVORelay-dep-whisper-rs`.

## Release Body Drafting

When preparing release text for user review:
1. Start only after user confirms that a new release body draft is needed.
2. Build a short, user-facing summary from commits between the previous tag and current release commit.
3. Exclude internal-only items (docs-only, sync logs, tooling-only chores) unless the user asks to include them.
4. Keep/update the baseline static notice text from the checked-in release body files:
   - `main`: `.github/release-notes/main.md`
   - `Microsoft-store`: `.github/release-notes/microsoft-store.md`
   - `cuda-integration`: `.github/release-notes/cuda.md`
5. GitHub Actions reads these Markdown files directly and prepends them ahead of `generate_release_notes: true`.
6. Present the final draft in chat and ask explicitly: use as-is or apply user-edited text from chat.
