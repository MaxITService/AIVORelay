Branch tags: #branch/main #branch/release-microsoft-store #branch/integration-cuda #branch/integration-combined

# All Branches Release Playbook

Use this playbook only when the user asks to prepare or publish releases on more than one branch for the same version line.

## Core Rule

Each branch has its own release contract:
- `main` -> tag `vx.y.z`
- `release/microsoft-store` -> tag `vx.y.z-store`, GitHub release must be published as a pre-release
- `integration/cuda` -> tag `vx.y.z-cuda`
- `integration/combined` -> follow its branch-specific release instructions if the user asks

Never assume that a rule from one branch automatically applies to another branch.

## Recommended Order For `release/microsoft-store` + `main`

If the same version is being released on both `release/microsoft-store` and `main`, and the `main` release body should link to the matching Store release:
1. Prepare and publish the `release/microsoft-store` GitHub release first.
2. Publish it as a pre-release.
3. Copy the final published Store release URL.
4. Insert that URL into `.github/release-notes/main.md`.
5. Only then finalize the `main` release body, commit, tag, and publish `main`.

This order exists so the `main` release body points to the exact same-version Store release, not a stale older one.

## When `main` Is Released Alone

If only `main` is being released, do not update the Microsoft Store link to some older unrelated release just to keep a link present.

## Multi-Branch Checklist

When the user asks for release work on multiple branches:
1. Identify the exact target branches and target version.
2. Read the branch-specific release doc for each requested branch.
3. Prepare branch-specific release bodies separately.
4. Keep tag names, release titles, and release assets branch-specific.
5. Ask before pushing or publishing anything if the user has not explicitly requested that step.

