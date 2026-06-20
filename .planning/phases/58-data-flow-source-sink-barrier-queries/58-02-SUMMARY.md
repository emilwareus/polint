# Phase 58-02 Summary: Provider Source Introduction and Capability Wiring

## Result

Completed.

The data-flow provider now introduces backed source edges from framework
trust-boundary models into matching MIR parameter places, and the `dataflow`
preview capability is supported for policy-query rules.

## Delivered

- Added deterministic `SourceIntroduction` edges from trust-boundary source
  models to matching function parameter places.
- Preserved stable edge keys, normalized output ordering, model references, and
  explicit evidence labels.
- Added a provider unit test proving source models connect to matching
  parameters.
- Moved `dataflow` from reserved preview capability handling to supported
  planning.
- Added `dataflow` to semantic pipeline trigger capabilities so rules that
  request `DataFlow<'_>` receive the needed provider facts.
- Updated capability and preview tests to reflect backed Phase 58 behavior.

## Verification

- `cargo test -p polint --lib source_models_create_source_introduction_edges_to_matching_parameters --locked` passed.
- `cargo test -p polint --lib policy_capabilities_report_phase_support_boundaries --locked` passed.
- `cargo test -p polint --test cli phase5 --locked` passed.
- Covered by the full library regression: `cargo test -p polint --lib --locked`
  passed with 2308 tests.

