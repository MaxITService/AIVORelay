# Fork Agents Guide

Branch tags: #branch/codex-combined

## Always-On Rules

- Do not push unless the user explicitly asks.
- If you are not confident that a change will fix the issue, stop and confirm before making a risky fix.
- Use proper commit messages.
- For this branch, use `main` as the only sync source.
- Read extra documentation only when the current task requires it.
- Use Obsidian-style links.
- Keep documentation short, dense, and in English.
- When committing, if encountered surprising behavior, like "made change and it does not build until we completely overhaul build scripts", propose edits to docs in chat. Dense, short, and to the point.

## Branch Scope

- Current branch: `codex/combined`
- Main branch: `main`
- This branch should stay close to `main` in normal app code.
- Keep branch-local changes limited to combined build, packaging, runtime-variant, and release behavior.

## Read Docs Only When Needed

- If you change the program code that is not branch-only specific behaviour, read `AGENTS.md` on `main` first. Ask the user if that file is not available.
- If you need combined branch-local build, sidecar packaging, bindings, or verification rules, read [[.AGENTS/build-combined|build-combined.md]].
- If you need combined branch-local runtime, packaging, or file-diff context, read [[.AGENTS/combined-branch-notes|combined-branch-notes.md]].
- If you prepare a combined release, read [[.AGENTS/Release-combined|Release-combined.md]].
- If you need current combined sync context or must record a completed `main -> codex/combined` sync, read [[.AGENTS/branch-log-maintenance|branch-log-maintenance.md]], [[.AGENTS/branch-propagation-log|branch-propagation-log.md]], and [[.AGENTS/branching-status|branching-status.md]].
- If you need local doc overview, read [[.AGENTS/MOC|MOC.md]].
- If you need local-only materials, read [[.AGENTS/.UNTRACKED/MOC|.UNTRACKED/MOC.md]].

At the start of a new session, include `AGENTS.md received.` once in your first normal reply.

Do not send it as a standalone placeholder message, and do this only once per session.
