# Main To integration/cuda Propagation Playbook
Branch tags: #branch/main #branch/integration-cuda

This playbook contains specific rules for `main -> integration/cuda` propagation.
For the general workflow, start point rules, and documentation handling, you MUST follow:
- [[.AGENTS/general-branch-propagation-playbook|general-branch-propagation-playbook.md]]

## Propagation Scope

Target branch: `integration/cuda`

**Default exclusions for CUDA Edition:**
- documentation changes (General Rule applied)
- Microsoft Store-specific changes
- branch-local CUDA dependency/release wiring changes that only exist on `integration/cuda`

**Allowed by default:**
- normal runtime fixes from `main`
- UI fixes from `main`
