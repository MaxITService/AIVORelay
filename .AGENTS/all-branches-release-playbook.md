Branch tags: #branch/main #branch/release-microsoft-store

# All Branches Release Playbook

Use this playbook only when the user asks to prepare or publish releases on more than one branch for the same version line.

## Core Rule

Each branch has its own release contract:
- `main` -> tag `vx.y.z`
- `release/microsoft-store` -> no own release or tag; `release.yml` on `main` adds its `aivorelay-store_*` files to the `vx.y.z` draft

Never assume that a rule from one branch automatically applies to another branch.

## Order For `release/microsoft-store` + `main`

1. Bump both branches to the same version and push `release/microsoft-store` first.
2. Run `Release` on `main`; it refuses a Store head with another version or with updater artifacts.

## Multi-Branch Checklist

When the user asks for release work on multiple branches:
1. Identify the exact target branches and target version.
2. Read the branch-specific release doc for each requested branch.
3. Prepare branch-specific release bodies separately.
4. Keep tag names, release titles, and release assets branch-specific.
5. Ask before pushing or publishing anything if the user has not explicitly requested that step.
