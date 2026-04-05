# Main To Microsoft-store Propagation Playbook
Branch tags: #branch/main #branch/microsoft-store

This playbook contains specific rules for `main -> Microsoft-store` propagation.
For the general workflow, start point rules, and documentation handling, you MUST follow:
- [[.AGENTS/general-branch-propagation-playbook|general-branch-propagation-playbook.md]]

## Propagation Scope

Target branch: `Microsoft-store`

**Default exclusions for Microsoft Store Edition:**
- documentation changes (General Rule applied)
- self-update / auto-update changes
- AVX512-only changes

**Allowed by default:**
- AVX2-targeted changes
- normal runtime fixes from `main`
- UI fixes from `main`
