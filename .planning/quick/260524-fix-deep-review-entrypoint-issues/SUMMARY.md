---
status: complete
created: 2026-05-24
completed: 2026-05-24
workflow: gsd-quick
---

# Fix Deep Review Entrypoint Issues Summary

## Changes

- Constrained TypeScript source-level fallback recognition to known Express/MCP receiver variables and ignored matches inside comments or string/template literals.
- Added Express `app.route("/path").get(handler)` source fallback support with path and method metadata preserved.
- Switched Go test entrypoint recognition to use the Go adapter's `FunctionFact::is_test` classification instead of prefix-only matching.
- Added observed entrypoint handler names to eval payloads plus deterministic handler/trigger invariants for representative Go, Express, and MCP fixture entrypoints.
- Added regression tests for source fallback false positives, route chains, and Go test over-reporting.

## Validation

- `cargo fmt --all --check`
- `cargo test -p polint --lib entrypoints -- --nocapture`
- `cargo test -p polint --lib eval_framework_entrypoints_core_fixture_passes -- --nocapture`
- `cargo clippy -p polint -- -D warnings`
