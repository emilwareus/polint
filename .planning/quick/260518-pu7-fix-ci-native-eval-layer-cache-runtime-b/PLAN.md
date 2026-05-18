# Quick Task 260518-pu7: Fix CI Native Eval Layer-Cache Runtime Budget Failure

## Goal

Make the layer-cache native eval fixture pass reliably on CI while keeping its runtime budget check meaningful.

## Diagnosis

The attached CI logs fail `eval_layer_cache_fixture_passes` and the suite category coverage test because the layer-cache fixture exceeds its 5 second runtime budget on GitHub runners. The fixture runs cold, warm, disabled, and import-edit analysis passes across real cache providers, so the previous budget reflected local-machine speed rather than a portable CI envelope.

Observed CI runtimes were roughly 25 seconds on macOS, 31 seconds on Ubuntu, and 65 seconds on Windows.

## Scope

- Raise only the layer-cache fixture runtime budget to 90 seconds.
- Leave cache hit, miss, bypass, eviction, and coverage invariants unchanged.
- Leave other eval fixture budgets unchanged.

## Verification

- `cargo test -p polint --lib eval_layer_cache_fixture_passes --locked`
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked`
