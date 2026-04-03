# Fork Agents Guide

Branch tags: #branch/microsoft-store

## Always-On Rules

- Do not push unless the user explicitly asks.
- If you are not confident a change will fix the issue, stop and confirm before making a risky fix.
- Use proper commit messages.
- For this branch, use `main` as the only sync source.
- Keep documentation short, dense, and in English.
- Use Obsidian-style links.
- When committing, if encountered surprising behavior, like "made change and it does not build until we completely overhaul build scripts", propose edits to docs in chat. Dense, short, and to the point.

## Branch Scope

- This branch is the Microsoft Store Edition.
- Keep Store-specific behavior compliant with Microsoft Store policies.
- For non-branch-specific program changes, read `AGENTS.md` on `main` first.
- If `AGENTS.md` on `main` is not available, ask the user.

## Local Docs

- [[.AGENTS/MOC|MOC.md]]
- [[.AGENTS/build-store|build-store.md]]
- [[.AGENTS/Release-store|Release-store.md]]
- [[.AGENTS/store-branch-notes|store-branch-notes.md]]
- [[.AGENTS/branch-propagation-log|branch-propagation-log.md]]
- [[.AGENTS/branching-status|branching-status.md]]
- [[.AGENTS/store-sync-maintenance|store-sync-maintenance.md]]
- [[.AGENTS/.UNTRACKED/MOC|.UNTRACKED/MOC.md]]

At the start of a new session, include `AGENTS.md received.` once in your first normal reply.

Do not send it as a standalone placeholder message, and do this only once per session.
