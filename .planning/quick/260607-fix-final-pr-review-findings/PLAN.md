---
quick_id: 260607-fix-final-pr-review-findings
slug: fix-final-pr-review-findings
status: completed
created: 2026-06-07
description: Fix final PR review findings for computed property call-flow work
---

# Quick Task 260607: Fix Final PR Review Findings

## Objective

Fix the three final review warnings before merge:

- Route assignments to known setter properties through setter calls instead of
  overwriting them as ordinary callable properties.
- Preserve bounded computed assignment key unions.
- Keep simple constant computed class method names consistent across TS adapter,
  MIR lowering, TS inventory, and TS object-model extraction.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::inventory --lib --locked`
- `cargo test -p polint ts::object_model --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `make lint`
