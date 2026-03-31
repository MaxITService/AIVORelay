# Dev Build Speed Notes

Latest measured on `main` on 2026-03-31 against the real `Dev-AivoRelay` flow:

- scenario: start `Dev-AivoRelay`, wait for first app launch, edit one tiny Rust file, measure `save -> new aivorelay.exe usable again`
- baseline: `23.74s`
- `lld-link.exe`: `21.30s`
- `lld-link.exe` + `CARGO_PROFILE_DEV_DEBUG=line-tables-only`: `11.16s`
- `lld-link.exe` + `CARGO_PROFILE_DEV_DEBUG=limited`: `11.10s`
- `lld-link.exe` + `CARGO_PROFILE_DEV_DEBUG=none`: `10.36s`

Current safest fast path:

- prefer `lld-link.exe`
- if backtraces with file/line are enough, `line-tables-only` is a large win
- if some extra debug metadata is still desired, `limited` was nearly identical in speed and is the best compromise
- `none` is fastest, but least friendly for debugger-heavy work

What did **not** look promising before extra testing:

- Cargo already defaults `build.jobs` to logical CPU count
- Cargo dev already defaults to `incremental = true`
- Cargo dev already defaults to `codegen-units = 256`
- `sccache` is a weak fit for this exact loop because incrementally compiled crates are not cached, and linker-invoking bins are also not cached

Extra measured follow-up on 2026-03-31 using `lld-link.exe` + `CARGO_PROFILE_DEV_DEBUG=limited` as the base:

- `CARGO_BUILD_JOBS`:
  - default: `11.84s`
  - `8`: `11.89s`
  - `16`: `11.45s`
  - `32`: `11.65s`
  - `64`: `11.50s`
- conclusion: forcing more Cargo jobs above the machine's 16 logical CPUs did not materially help the incremental dev loop

Extra measured `codegen-units` follow-up on 2026-03-31 using the same base:

- default dev codegen-units: `11.11s`
- forced `64`: `13.61s`
- forced `256`: `13.55s`
- forced `512`: `13.37s`
- conclusion: Cargo's default dev setup was better than forcing explicit codegen-unit counts for this project

Extra safe config-only debuginfo follow-up on 2026-03-31:

- `lld-link + line-tables-only`: `11.16s`
- `lld-link + line-tables-only + [profile.dev.package."*"].debug = false`: `10.50s`
- `lld-link + limited`: `10.96s`
- `lld-link + limited + [profile.dev.package."*"].debug = false`: `10.45s`
- `lld-link + none`: `10.46s`
- `lld-link + none + [profile.dev.package."*"].debug = false`: `10.26s`

Best safe config-only compromise so far:

- `lld-link.exe`
- `CARGO_PROFILE_DEV_DEBUG=limited`
- `[profile.dev.package."*"] debug = false`

That kept build diagnostics, kept some debugger-friendliness for the workspace crate, and measured `10.45s` on the real incremental `Dev-AivoRelay` loop.

Extra `profile.dev.build-override` follow-up on 2026-03-31 using the best safe config baseline:

- baseline (`lld-link + limited + dependency debuginfo off`): `11.50s`
- add `[profile.dev.build-override] debug = false`: `11.42s`
- add `[profile.dev.build-override] opt-level = 0`: `18.91s`
- add both: `12.01s`

Conclusion:

- `build-override.debug = false` is basically noise-level for this incremental Rust edit loop
- `build-override.opt-level = 0` was actively worse here
- `build-override` does not look like a worthwhile tuning path for the user's main `edit -> relaunch app` scenario

Evidence:

- benchmark summary: `.AGENTS/.UNTRACKED/dev-aivorelay-theories-20260331-130124/summary.md`
- jobs summary: `.AGENTS/.UNTRACKED/dev-aivorelay-jobs-20260331-135034/summary.md`
- codegen summary: `.AGENTS/.UNTRACKED/dev-aivorelay-codegen-20260331-140404/summary.md`
- safe config summary: `.AGENTS/.UNTRACKED/dev-aivorelay-safe-config-20260331-152556/summary.md`
- build-override summary: `.AGENTS/.UNTRACKED/dev-aivorelay-build-override-20260331-173502/summary.md`
