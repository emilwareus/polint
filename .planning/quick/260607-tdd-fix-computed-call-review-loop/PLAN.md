---
quick_id: 260607-tdd-fix-computed-call-review-loop
slug: tdd-fix-computed-call-review-loop
status: completed
created: 2026-06-07
description: TDD fix computed call review findings and iterate review loop
---

# Quick Task 260607: TDD Fix Computed Call Review Loop

## Objective

Use TDD to fix the latest review findings, then re-review and iterate until no
new actionable findings remain:

- Numeric computed object keys should resolve for assignments and calls.
- Computed union getter calls should merge receiver side effects without losing
  earlier candidate effects.

## Process

1. Add failing regression tests for the reviewed findings.
2. Run focused tests and confirm failures.
3. Implement the smallest production fixes.
4. Run targeted tests and `make lint`.
5. Re-review the touched code and repeat if new findings appear.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::inventory --lib --locked`
- `cargo test -p polint ts::object_model --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `make lint`
