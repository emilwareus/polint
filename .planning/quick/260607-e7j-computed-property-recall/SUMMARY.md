# Quick Task 260607-e7j Summary

## Outcome

Implemented bounded computed-property recall for the Jelly computed-properties
fixture while keeping the model local and deterministic.

## Changes

- Added bounded constant evaluation for computed property keys from string
  literals, string concatenation, simple conditionals, boolean constants, and
  string array index reads.
- Applied computed key names to object literals, class fields, and class
  methods.
- Restored callable identities for simple constant computed class methods in
  TS function facts and TS MIR lowering.
- Split getter/setter property targets from ordinary callable properties and
  modeled getter reads as receiver-bound calls.
- Added a real TS pipeline regression derived from Jelly's
  `computedProperties.js`.

## Measured Performance

- Jelly overall: **814 / 609 / 665** to **840 / 604 / 639** (TP / FP / FN).
- Jelly precision: **57.20%** to **58.17%**.
- Jelly recall: **55.04%** to **56.80%**.
- Jelly F1: **56.10%** to **57.48%**.
- `tests/approx/computedProperties.json`: **8 / 4 / 18** to **24 / 3 / 2**.

## Verification

- PASS `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- PASS `cargo test -p polint ts::tests --lib --locked`
- PASS `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- PASS release external graph benchmark
- PASS `make lint`
