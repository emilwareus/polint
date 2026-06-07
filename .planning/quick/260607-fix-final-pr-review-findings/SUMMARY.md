# Quick Task 260607: Fix Final PR Review Findings

## Result

Fixed the three final PR review findings:

- Setter assignments now invoke known setter targets and merge `this` side effects
  back into the receiver instead of overwriting accessor properties as ordinary
  callable data properties.
- Computed assignment keys now preserve bounded constant unions, so assignments
  like `obj[unknown ? "left" : "right"] = fn` flow to both candidate properties.
- TS inventory and object-model extraction now agree with adapter/MIR naming for
  simple constant computed method keys such as `["na" + "me"]`.

## Regression Coverage

- Added a real TS pipeline regression for setter side effects without accessor
  overwrite.
- Added a real TS pipeline regression for computed assignment key unions.
- Added inventory and object-model unit tests for constant computed class method
  names/property writes.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::inventory --lib --locked`
- `cargo test -p polint ts::object_model --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `make lint`
