# Quick Task 260607: Fix Second Review Findings

## Result

Fixed the follow-up review findings:

- Computed object-property calls now resolve object data properties and accessors
  for constant computed keys, including bounded key unions.
- Getter-returned callables are now invoked with the outer call arguments, so
  flows through `obj.cb(fn)` and `obj["cb"](fn)` bind `fn` into the returned
  function body.
- Nested string-concatenated computed method names are now recognized consistently
  across TS adapter, MIR lowering, TS inventory, and TS object-model extraction.

## Regression Coverage

- Added a real TS pipeline regression for computed object-property calls.
- Added a real TS pipeline regression for getter-returned callable argument flow.
- Extended computed method-name tests to cover nested string concatenation.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::inventory --lib --locked`
- `cargo test -p polint ts::object_model --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `make lint`
