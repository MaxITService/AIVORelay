# CUDA Sync Maintenance

Read this file only when recording a completed `main -> cuda-integration` sync.

## After A Successful Sync

1. Update [[.AGENTS/branch-propagation-log|branch-propagation-log.md]] in this branch.
2. Update [[.AGENTS/branching-status|branching-status.md]] in this branch.
3. Refresh [[.AGENTS/cuda-docs-main-diff-manifest|cuda-docs-main-diff-manifest.md]] if the branch-local file set changed.
4. Refresh [[.AGENTS/cuda-docs-branch-overview|cuda-docs-branch-overview.md]] or [[.AGENTS/cuda-docs-model-runtime-notes|cuda-docs-model-runtime-notes.md]] if branch-local behavior changed.
5. Mirror the same log and cursor update back into the documentation on `main`.

## Verification Rule

Before writing the cursor, verify the real sync point with git history.
Do not trust docs alone as the source of truth.

Useful commands:

```bash
git log cuda-integration --oneline
git cherry -v cuda-integration main
git reflog cuda-integration
```

## Log Rules

- Keep newest entries first.
- Keep only the last 10 entries.
- Keep issue notes very short.
