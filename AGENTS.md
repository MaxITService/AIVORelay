# CUDA Branch Agent Guide

This is the entry file for AI/code agents working in the `cuda-integration` branch.

## Always-On Rules

- Do not push unless the user explicitly asks.
- If you are not confident that a change will fix the issue, stop and confirm before making a risky fix.
- Use proper commit messages.
- Read extra documentation only when the current task requires it.
- Use Obsidian-style links.
- rg, sg, Python and lots of dev tools are installed on this Windows machine.

## Branch Scope

Current branch:
- `cuda-integration`

Main branch:
- `main`

This branch should stay close to `main` in normal app code.
Keep branch-local changes limited to CUDA-specific build, dependency, release, and packaging behavior.

## Read Docs Only When Needed

- If you change the program code that is not branch-only specific behaviour, read `AGENTS.md` on `main` first. Ask the user if that file is not available.
- If you need fork-wide code notes, read `code-notes.md` on `main`.
- If you need CUDA branch-local code, dependency, or packaging context, read [[.AGENTS/cuda-branch-notes|cuda-branch-notes.md]].
- If you need CUDA build, toolchain, bindings, or verification rules, read [[.AGENTS/build-cuda|build-cuda.md]].
- If you prepare a CUDA release, read [[.AGENTS/Release-cuda|Release-cuda.md]].
- If you need current CUDA branch sync context or must record a completed `main -> cuda-integration` sync, read [[.AGENTS/branch-log-maintenance|branch-log-maintenance.md]], [[.AGENTS/branch-propagation-log|branch-propagation-log.md]], and [[.AGENTS/branching-status|branching-status.md]].
- If you need branch-local doc overview, read [[.AGENTS/MOC|MOC.md]].

## Documentation Style

- Keep this file short.
- Keep detailed procedures in linked documents.
- Keep documentation text in English.
- When committing, if encountered surprising behavior, like "made change and it does not build until we completely overhaul build scripts", propose edits to docs in chat. Dense, short, and to the point.

At the start of a new session, include `AGENTS.md received.` once in your first normal reply.

Do not send it as a standalone placeholder message, and do this only once per session.
