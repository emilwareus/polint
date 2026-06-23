# Summary: Policy Query Review Fixes

Completed a multi-pass review/fix loop on the v1.4 policy query surface.

## Fixed

- Policy capability planning now requests provider prerequisites for events,
  calls, control-flow, and data-flow rules.
- Data-flow query behavior now preserves distinct sink call sites, honors
  precision filtering for found paths, keeps unknown/budget/unsupported rows
  visible, treats heuristic sources/sinks truthfully, and handles internal
  synthetic call targets without hiding syntax labels.
- Trust-boundary source modeling now targets Go HTTP request parameter index 1,
  keeps Go middleware ambiguous, and identifies TS/JS error-middleware request
  parameters from MIR parameter places.
- Control-flow policy results no longer overclaim exactness and use a total
  deterministic ordering for mixed CFG/source ordering.
- Reachability queries surface unresolved/budget/unsupported matching targets
  at default precision instead of silently treating uncertainty as absence.
- Unknown taxonomy now includes policy/data-flow capability unknowns in
  consolidated reporting, and `inspect unknowns` requests the same public
  unknown capabilities it reports.
- Generated rule scaffolds now reject unsupported policy template/language
  combinations, produce runnable generic fixtures, preserve parser diagnostics
  under `polint test --only-rule`, and align template severities/docs.
- Skill text and docs now match the supported template matrix and precision
  semantics.

## Review Loop

- Ran multiple subagent review passes over implementation, CLI/docs, unknowns,
  and rule-author ergonomics.
- Fixed all actionable findings from those passes.
- Final two read-only reviewers returned no findings.

## Verification

- `cargo fmt --all`
- `git diff --check`
- `cargo test -p polint --lib policy_queries::tests --locked`
- `cargo test -p polint --lib analysis::entrypoints::trust_boundaries::tests --locked`
- `cargo test -p polint --lib analysis::unknown_taxonomy::collect::tests --locked`
- `cargo test -p polint --test cli unknowns --locked`
- `cargo test -p polint --test cli new_rule_ --locked`
- `cargo test -p polint --test cli phase --locked`
- `cargo test -p polint --locked`
