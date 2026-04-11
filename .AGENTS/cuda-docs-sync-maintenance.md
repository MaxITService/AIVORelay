Branch tags: #branch/integration-cuda

# CUDA Sync Maintenance

Read this file only when recording a completed `main -> integration/cuda` sync.
This branch does not keep a full propagation log or global status. `main` tracks all branches.

## After A Successful Sync

1. Refresh [[.AGENTS/cuda-docs-main-diff-manifest|cuda-docs-main-diff-manifest.md]] if the branch-local file set changed.
2. Refresh [[.AGENTS/cuda-docs-branch-overview|cuda-docs-branch-overview.md]] or [[.AGENTS/cuda-docs-model-runtime-notes|cuda-docs-model-runtime-notes.md]] if branch-local behavior changed.
3. Record the successful sync on `main` (update `branch-propagation-log.md` and `branching-status.md` in the `main` branch).

## Verification Rule

Before writing the cursor, verify the real sync point with git history.
Do not trust docs alone as the source of truth.

Useful commands:

```bash
git log integration/cuda --oneline
git cherry -v integration/cuda main
git reflog integration/cuda
```
