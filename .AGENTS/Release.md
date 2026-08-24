# Release Rules
Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

## GitHub Actions / Workflow YAML

Do not modify `.github/workflows/*.yml` unless the user explicitly asks for it.

These files are fragile. A broken workflow file can make GitHub Actions stop showing or running the expected workflow cleanly.

Rules:
1. Never edit workflow YAML without explicit user approval.
2. Never add new inputs or parameters to reusable workflows such as `.github/workflows/build.yml` unless the user explicitly asks for that contract change.
3. Inside YAML `script: |` blocks, every content line must stay indented consistently.
4. Avoid multi-line JavaScript template literals inside YAML `script: |` blocks; prefer plain strings with `\n` or simple concatenation.
5. If workflow YAML was edited, validate the file before committing when the user asks for verification.

## Fast Draft Release Path

If the user explicitly asks for a full draft release flow such as "do the release", "prepare the release", or "bump, push, and run the release workflow", treat that as approval to execute the whole draft-release path end-to-end for the requested branches.
In that fast path, do not stop to re-ask about release body draft, push, or workflow dispatch; prepare sensible release notes from the user-facing commits, keep the release as a draft unless the user explicitly asks to publish, and only stop if there is branch/version ambiguity, unusual risk, or a real conflict.

## Version Bump

When asked to bump the app version:
1. Update `"version": "x.y.z"` in `package.json`.
2. Update `"version": "x.y.z"` in `src-tauri/tauri.conf.json`.
3. Update `version = "x.y.z"` in `src-tauri/Cargo.toml`.
4. In the same version-preparation step, detect the previous release tag for the branch (`vx.y.z` for `main`, `vx.y.z-store` for `release/microsoft-store`, `vx.y.z-cuda` for `integration/cuda`) and review the user-facing commits since that tag once. Use that review to prepare both the in-app **What's New** content and the GitHub release body.
5. Refresh the in-app **Help -> What's New** section for the target release:
   - set `help.whatsNew.since` in `src/i18n/locales/en/translation.json` and `src/i18n/locales/ru/translation.json` to the previous released version;
   - replace `help.whatsNew.items` in both locale files with short, user-facing highlights added since that version;
   - keep `WHATS_NEW_ITEMS` in `src/components/settings/help/HelpSettings.tsx` aligned with the new translation keys and their intended display order;
   - remove stale item keys instead of accumulating an indefinite release history in this section.
6. At the same time, prepare a short release body draft from the same commit review:
   - include only end-user facing changes in short bullets;
   - keep the standard static notice text that is normally used in release body.
7. Show the in-app **What's New** copy and the release body draft together in chat and explicitly ask whether to use them as-is or apply user edits. The [[#Fast Draft Release Path|fast draft release path]] remains the exception and does not require a separate approval round.
8. After approval, write the release body into the matching checked-in file before commit:
    - `main`: `.github/release-notes/main.md`
    - `release/microsoft-store`: `.github/release-notes/microsoft-store.md`
    - `integration/cuda`: `.github/release-notes/cuda.md`
9. If `main` release text should point users to a same-version `release/microsoft-store` release, you may predict the final Store release URL as `https://github.com/MaxITService/AIVORelay/releases/tag/vx.y.z-store` and place it into `.github/release-notes/main.md` before the Store release exists.
10. If the Store release already exists, prefer verifying that the predicted URL resolves and keep the checked-in note aligned with the final GitHub release body.
11. If `main` is being released on its own, do not refresh the Microsoft Store link just to point at an older unrelated release.
12. Stop before commit and ask the user to run the build/check flow on their side (user-driven verification).
13. After user build is done, re-check git status. If `src-tauri/Cargo.lock` changed due version bump, include it in the same version-bump commit.
14. Do not run build or verification commands unless the user explicitly asks. This repo expects build verification to be user-driven.
15. Commit the version files, approved in-app **What's New** update, release body, and any version-generated lockfile change together with `chore: bump version to x.y.z`.
16. After commit, ask whether to create tag and push now.

## Main Release Preflight

Before tagging or running `Release` for `main`, verify `.github/release-notes/main.md` is current for the target version.
After pushing `main`, run `gh workflow run release.yml --ref main`; the workflow creates a draft GitHub release named from `src-tauri/tauri.conf.json`.

## Tags And Branches

Use the same numeric app version on both release branches.

For `main`:
1. Tag `vx.y.z` only when the user explicitly asks.
2. Push `main` and the tag only when the user explicitly asks to push.

For `release/microsoft-store`: if asked, prepare this branch too. If not asked, skip it entirely.
1. Keep the app version numeric, for example `0.9.1`. Same number is released in both branches, but only by user's apporval.
2. Use the Microsoft Store release workflow and store-specific release naming. Its GitHub release must be published as a pre-release.
3. Tag `vx.y.z-store` only when the user explicitly asks for the store tag.
4. Push `release/microsoft-store` and the store tag only when the user explicitly asks.

For `integration/cuda`: if asked, prepare this branch too. If not asked, skip it entirely.
1. Keep the app version numeric, for example `0.9.1`. Same number may be reused, but only by user's approval.
2. Use the CUDA release workflow and CUDA-specific release naming.
3. Tag `vx.y.z-cuda` only when the user explicitly asks for the CUDA tag.
4. Push `integration/cuda` and the CUDA tag only when the user explicitly asks.
5. The CUDA release workflow builds a portable zip and expects the dependency repos `MaxITService/AIVORelay-dep-transcribe-rs` and `MaxITService/AIVORelay-dep-whisper-rs`.

## Release Body Drafting

When preparing release text for user review:
1. During a version bump, prepare the release body alongside the in-app **What's New** update from the same commit review. If release text is requested separately, start only after the user confirms that a new release body draft is needed.
2. Build a short, user-facing summary from commits between the previous tag and current release commit.
3. Exclude internal-only items (docs-only, sync logs, tooling-only chores) unless the user asks to include them.
4. Keep/update the baseline static notice text from the checked-in release body files:
   - `main`: `.github/release-notes/main.md`
   - `release/microsoft-store`: `.github/release-notes/microsoft-store.md`
   - `integration/cuda`: `.github/release-notes/cuda.md`
5. GitHub Actions reads these Markdown files directly and prepends them ahead of `generate_release_notes: true`.
6. Present the final draft in chat and ask explicitly: use as-is or apply user-edited text from chat.
7. Always mention upstream intakes included since the previous release tag. Keep this to one short, user-facing bullet, for example: "Included the latest upstream Handy improvements and fixes."

If a release was already created with stale body text, do not move tags; ask before editing a public release, and update the checked-in release-notes file separately if it should become the future baseline.
