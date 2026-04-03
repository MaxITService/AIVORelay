# Fork Agents Guide

This is the entry file for AI/code agents working with this fork.

## Always-On Rules

- Do not push unless the user explicitly asks.
- If you are not confident that a change will fix the issue, stop and confirm before making a risky fix.
- Use proper commit messages.
- For work on non-`main` branches, use `main` as the only sync source.
- During `main` -> release-branch propagation, never carry documentation by default. If a documentation change looks extraordinarily necessary to propagate, stop and ask the user before including it.
- Read additional documentation only when the current task requires it.
- Use Obsidian-style links.
- rg, sg, Python and lots of dev tools are installed on this Windows machine.
- something isane in user's request? Ask user first!

## Active Branches

- `main`
- `Microsoft-store`
- `cuda-integration`
- `codex/combined`

Relative worktree paths:

- `main` -> `.`
- `Microsoft-store` -> `../AIVORelay-ms-prop`
- `cuda-integration` -> `../worktree/cuda-integration`
- `codex/combined` -> `../../../AivoRelay`

When the user says "all branches", they currently mean these four branches.

## Read Extra Docs Only When Needed

- If you modify fork-specific code or add fork features, read [[.AGENTS/code-notes|code-notes.md]].
- If you need build, toolchain, bindings, or verification rules, read [[.AGENTS/build-environment|build-environment.md]].
- If you do `upstream/main -> main` intake, read [[.AGENTS/upstream-intake-playbook|upstream-intake-playbook.md]].
- If you propagate `main` into a release branch, read the matching playbook:
  - [[.AGENTS/main-to-microsoft-store-propagation-playbook|main-to-microsoft-store-propagation-playbook.md]]
  - [[.AGENTS/main-to-cuda-propagation-playbook|main-to-cuda-propagation-playbook.md]]
  - [[.AGENTS/main-to-combined-propagation-playbook|main-to-combined-propagation-playbook.md]]
- If you work on release or version preparation, read [[.AGENTS/Release|Release.md]].
- If you need branch state context, read [[.AGENTS/branching-status|branching-status.md]].
- If you need tracked docs navigation, read [[.AGENTS/MOC|.AGENTS/MOC.md]].
- If you need local-only notes, research, logs, scripts, reports, or artifacts that should not be propagated to GitHub, read [[.AGENTS/.UNTRACKED/MOC|.AGENTS/.UNTRACKED/MOC.md]].

## Documentation Style

- Keep this file short.
- Keep detailed procedures in linked documents.
- Keep documentation text in English.
- When committing, if encoutered surprising behavior, like "made change and it does not builds untill we completely overhaul build scripts", propose edits to docs in chat. Dense, short, and to the point.

At the start of a new session, include `AGENTS.md received.` once in your first normal reply.

Do not send it as a standalone placeholder message, and do this only once per session.
