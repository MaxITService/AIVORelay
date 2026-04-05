# Main To cuda-integration Propagation Playbook
Branch tags: #branch/main #branch/cuda-integration

This playbook contains specific rules for `main -> cuda-integration` propagation.
For the general workflow, start point rules, and documentation handling, you MUST follow:
- [[.AGENTS/general-branch-propagation-playbook|general-branch-propagation-playbook.md]]

## Propagation Scope

Target branch: `cuda-integration`

**Default exclusions for CUDA Edition:**
- documentation changes (General Rule applied)
- Microsoft Store-specific changes
- branch-local CUDA dependency/release wiring changes that only exist on `cuda-integration`

**Allowed by default:**
- normal runtime fixes from `main`
- UI fixes from `main`
