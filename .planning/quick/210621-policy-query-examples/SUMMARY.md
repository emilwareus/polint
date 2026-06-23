---
status: complete
task: policy-query-examples
---

# Policy Query Examples Final Pass Summary

## Completed

- Added realistic rule/application examples to `docs/facts/policy-queries.md`
  for data-flow, control-flow guard, and reachable-call policies.
- Improved generated policy-template fixtures with comments that explain the
  safe and violating code paths users are analyzing.
- Updated generated data-flow starters to use bounded `max_depth = 24` and
  `max_paths = 128`, which keeps the included HTTP request fixtures truthful
  without hiding budget uncertainty.
- Added regression assertions that data-flow templates retain those bounded
  query caps.

## Verification

- `cargo test -p polint --test cli new_rule_policy_template --locked`
- `git diff --check`
- `make lint`
- `cargo test -p polint --locked`
