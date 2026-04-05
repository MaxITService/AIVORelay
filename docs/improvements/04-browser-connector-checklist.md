# Feature 04: Browser Connector Checklist

## Goal

Make the browser connector setup feel more guided by surfacing the three most important installation milestones right on the page.

## What changed

- Added a quick checklist card to the Browser Connector settings page.
- Shows live readiness for server enablement, export-path selection, and first extension contact.
- Turns a long instruction flow into an at-a-glance status summary.

## Verification

- `bun x tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml` with the shared Windows build-env helper and a short `CARGO_TARGET_DIR`
