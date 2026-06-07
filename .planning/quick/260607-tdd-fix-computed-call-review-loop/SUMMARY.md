# Quick Task 260607: TDD Fix Computed Call Review Loop

## Result

Fixed the latest review findings and one additional accessor-flow issue found in
the requested review loop:

- Numeric computed object keys now resolve through the same computed member
  property-name helper used by assignments and calls.
- Computed union getter calls now refresh receiver state between candidates, so
  side effects from earlier candidates are preserved.
- Getter bodies now preserve receiver side effects before returned callables are
  invoked.

## TDD Evidence

- Added numeric computed key and union getter side-effect regressions first.
  `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
  failed on exactly those two tests.
- After fixing those, review found getter-body receiver side effects were still
  dropped. Added a regression first; the same focused test command failed on
  that test.
- Implemented the getter invocation fix and reran the focused suite green.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::inventory --lib --locked`
- `cargo test -p polint ts::object_model --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `make lint`

Final review loop status: no new actionable findings in the touched code.
