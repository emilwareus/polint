# Phase 61-02 Summary: External SDK Matrix Tests

## Completed

- Added `phase61_policy_preview_external_sdk_matrix`, a temp-repo style test that writes `.polint/rules`, imports only `polint::sdk::prelude::*`, registers through `polint::runner::run_cli`, and runs `polint check --format json --fail-on none`.
- Covered every preview view and query family:
  - `Events<'_>::matching`
  - `Calls<'_>::forbidden_reachable`
  - `ControlFlow<'_>::missing_guard`
  - `ControlFlow<'_>::missing_cleanup`
  - `DataFlow<'_>::forbidden`
- Asserted normalized policy evidence for every result and no `polint/capability` diagnostics for supported preview views.

## Notes

- The test uses real Go and TypeScript source files so the public SDK path consumes real analysis facts rather than synthetic internal fixtures.

