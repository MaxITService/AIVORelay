# Main To integration/combined Propagation Playbook
Branch tags: #branch/main #branch/integration-combined

This playbook contains specific rules for `main -> integration/combined` propagation.
For the general workflow, start point rules, and documentation handling, you MUST follow:
- [[.AGENTS/general-branch-propagation-playbook|general-branch-propagation-playbook.md]]

## Propagation Scope

Target branch: `integration/combined`

**Default exclusions for Combined Edition:**
- documentation changes (General Rule applied)
- Microsoft Store-only updater policy changes
- branch-local multi-exe packaging changes that exist only on `integration/combined`

**Allowed by default:**
- shared runtime fixes from `main`
- UI fixes from `main`
- settings fixes from `main`
