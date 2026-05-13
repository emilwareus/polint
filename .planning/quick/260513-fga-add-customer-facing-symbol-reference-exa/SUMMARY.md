---
quick_id: 260513-fga
task: add-customer-facing-symbol-reference-exa
status: completed
date: 2026-05-13
---

# Summary

Added two customer-facing runnable examples for symbol/reference facts.

## Added

- `examples/ts-no-raw-api-calls`
  - Demonstrates a rule that requires generated SDK clients instead of raw API calls.
  - Shows a resolved local `rawRequest` call and an honest unresolved global `fetch` call.
- `examples/go-sensitive-writes`
  - Demonstrates a rule that blocks writes to a sensitive Go field outside approved files.
  - Shows exact semantic `Write` and `ReadWrite` references to `Account.Balance`.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures --locked`
- `cargo test -p polint --test cli ts_no_raw_api_calls_example_reports_resolved_and_unresolved_calls --locked`
- `cargo test -p polint --test cli go_sensitive_writes_example_reports_write_and_readwrite_references --locked`
