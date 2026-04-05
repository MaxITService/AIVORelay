# Main To codex/combined Propagation Playbook
Branch tags: #branch/main #branch/codex-combined

This playbook contains specific rules for `main -> codex/combined` propagation.
For the general workflow, start point rules, and documentation handling, you MUST follow:
- [[.AGENTS/general-branch-propagation-playbook|general-branch-propagation-playbook.md]]

## Propagation Scope

Target branch: `codex/combined`

**Default exclusions for Combined Edition:**
- documentation changes (General Rule applied)
- Microsoft Store-only updater policy changes
- branch-local multi-exe packaging changes that exist only on `codex/combined`

**Allowed by default:**
- shared runtime fixes from `main`
- UI fixes from `main`
- settings fixes from `main`
