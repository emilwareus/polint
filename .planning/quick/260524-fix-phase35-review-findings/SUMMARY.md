# Fix Phase 35 Review Findings Summary

Status: complete

## Changes

- Fixed Go and TS/JS framework recognizers to resolve handler entrypoints from registration arguments and source call text instead of defaulting to the caller function.
- Added source-level TS/JS framework registration recognition for top-level Express and MCP calls that do not currently appear as function-owned call facts.
- Expanded entrypoint provider output digest coverage to include the serialized payloads for entrypoints, trust boundaries, dispatch edges, and unresolved frameworks.
- Replaced provider failure diagnostics with a generic public message so internal framework marker strings do not leak into public JSON.
- Strengthened the framework entrypoint eval fixture with behavior assertions across Go, TypeScript, Express, MCP, trust boundaries, dispatch edges, and determinism.
- Cleaned warnings that blocked `cargo clippy -p polint -- -D warnings`.

## Validation

- `cargo fmt --all --check`
- `cargo test -p polint --lib entrypoints -- --nocapture`
- `cargo test -p polint --lib eval_framework_entrypoints_core_fixture_passes -- --nocapture`
- `cargo clippy -p polint -- -D warnings`
