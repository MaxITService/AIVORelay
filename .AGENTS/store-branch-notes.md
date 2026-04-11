Branch tags: #branch/release-microsoft-store

# Store Branch Notes

Intentional Microsoft Store Edition differences from `main`.

## Policy And Packaging

- Microsoft Store Edition must not rely on self-updating.
- Store release workflow disables normal binary signing because Store ingest signs the package.
- Store release work must stay compatible with sandboxed packaged app behavior.

## Runtime And Build Differences

- Keep AVX2 as the branch distribution baseline.
- Do not re-enable AVX512-only distribution assumptions for Store builds.
- Tauri updater endpoints stay disabled in this branch.

## UI Differences

- Do not restore frontend update-check UI in the Store branch.
- Store-specific window/app labeling may differ from `main` when needed for release clarity.

## Documentation Rule

- Shared program documentation is owned by `main`.
- Keep only branch-local notes and procedures here.
