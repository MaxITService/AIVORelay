# Fork Agents Guide
Branch tags: #branch/release-microsoft-store

## Always-On Rules

- Do not push unless the user explicitly asks.
- Do not run a build or tests unless the user explicitly asks.
- Do not run formatters in write mode or add formatting-only churn.
- If a request is unclear or a proposed fix is risky, stop and confirm first.
- Use proper commit messages.
- Use `main` as the only propagation source for this branch.
- Preserve Store-specific files instead of blindly overwriting them from `main`.
- Keep documentation short, dense, in English, and linked with Obsidian-style links.

## Branch Contract

- This branch produces the Microsoft Store Edition.
- [[.AGENTS/store-branch-notes|store-branch-notes.md]] is the canonical inventory of intentional differences from `main`.
- For shared program behavior, read the current `AGENTS.md` on `main` before editing.
- Never infer Store ARM64 support from ARM64 paths in the reusable build workflow; the Store release workflow currently publishes Windows x64 only.
- Never advertise this build as a fallback for CPUs without AVX2.

## Task Routing

- Build, toolchain, lockfile, bindings, or verification: [[.AGENTS/build-store|build-store.md]]
- Store release or version preparation: [[.AGENTS/Release-store|Release-store.md]]
- `main` propagation: [[.AGENTS/store-sync-maintenance|store-sync-maintenance.md]]
- Navigation: [[.AGENTS/MOC|MOC.md]]

At the start of a new session, include `AGENTS.md received.` once in the first normal reply, never as a standalone message.
