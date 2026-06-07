---
quick_id: 260607-fix-second-review-findings
slug: fix-second-review-findings
status: completed
created: 2026-06-07
description: Fix second review findings for TS computed/accessor call flows
---

# Quick Task 260607: Fix Second Review Findings

## Objective

Fix the follow-up review findings:

- Resolve computed object-property calls such as `obj["left"]()` through object
  properties and accessors, not only through numeric collection indexes.
- Invoke getter-returned callables with the outer call arguments so flows like
  `obj.cb(fn)` bind `fn` into the returned function.
- Keep computed method-name handling consistent across adapter, MIR, inventory,
  and object-model extraction for nested string concatenations.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::inventory --lib --locked`
- `cargo test -p polint ts::object_model --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `make lint`
