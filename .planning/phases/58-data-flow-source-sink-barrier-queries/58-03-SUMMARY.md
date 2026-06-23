# Phase 58-03 Summary: External Rule, Docs, and Closeout

## Result

Completed.

Phase 58 is proven through an outside-rule style CLI test, updated public docs,
updated generated skill text, and repository verification.

## Delivered

- Added a temp-repo CLI test where generated `.polint/rules` imports only
  `polint::sdk::prelude::*`, requests `DataFlow<'_>`, runs a `FlowQuery`, and
  asserts JSON diagnostic evidence.
- Updated `docs/facts/data-flow.md` with supported syntax, patterns, evidence,
  limits, heuristic behavior, and deferred categories.
- Updated capability-plan docs, API visibility docs, generated CLI skill text,
  and agent skill text to describe Phase 58 `DataFlow<'_>` behavior honestly.
- Updated requirements traceability, roadmap progress, and project state for
  Phase 58 completion.
- Ran focused, public-boundary, formatting, clippy, docs, facts-list, and broad
  library verification.

## Verification

- `cargo test -p polint --test cli phase58_data_flow_rule_reports_json --locked` passed.
- `cargo test -p polint --test public_surface_leak --locked` passed.
- `cargo fmt --all --check` passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed.
- `cargo doc -p polint --no-deps --locked` passed.
- `cargo run -p polint --locked -- facts list --format json` passed.
- `cargo test -p polint --lib --locked` passed with 2308 tests.

