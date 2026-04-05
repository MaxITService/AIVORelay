# Setup health check

The General page now includes a setup health check panel.

Why this helps:

- New users often configure several parts of the app, but still do not know what single missing piece is blocking a good first run.
- A compact readiness checklist turns that confusion into an actionable flow.
- Each item links directly to the section that can fix it.

Implementation notes:

- Added a computed readiness panel to `GeneralSettings`.
- It currently checks microphone selection, transcription path readiness, main shortcut presence, AI refinement readiness, and preview workflow status.
- The panel is intentionally practical: short status, short explanation, one button.
