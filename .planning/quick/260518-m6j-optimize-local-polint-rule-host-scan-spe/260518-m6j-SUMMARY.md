# Quick Task 260518-m6j Summary

## Outcome

Repo-local rule-host scans are substantially faster on the customer-shaped path.
Rule hosts now default to Cargo's optimized `release` profile, `--only-rule`
narrows the rule-host analysis plan, and the common single-host `polint check`
path avoids reloading source files in the parent process just to re-apply ignore
comments.

## Changes

- Added `POLINT_RULES_PROFILE` handling for local rule-host `cargo run`.
- Defaulted unset `POLINT_RULES_PROFILE` to `release`.
- Preserved `POLINT_RULES_PROFILE=dev` / `debug` for faster unoptimized rule-pack development and test runs.
- Applied `--only-rule` before rule-host plan construction so unrelated rule capabilities do not drive analysis work.
- Let a single local rule host apply ignore comments directly when check stats are not requested, avoiding a second parent-side source load.
- Documented the new environment variable in README and consumer setup docs.
- Set integration-test helpers to `POLINT_RULES_PROFILE=dev` so the test suite does not pay release build cost.

## Verification

- `cargo fmt --check`
- `cargo test -p polint local_rule_host_profile`
- `cargo test -p polint rule_plan_inputs_can_be_narrowed_by_only_rule_pattern`
- `cargo test -p polint --test cli check_only_rule_filters_out_diagnostics_when_pattern_matches_nothing`
- `cargo test -p polint --test cli capability_change_changes_cache_entries`
- `cargo test -p polint --test cli check_suppresses_next_line_ignore_and_ignores_reports_stats`
- `cargo build -p polint --release`
- Abuja customer-path smoke:
  - Before: `polint check --profile core --baseline --new-only --fail-on error --format json core/internal` = 12.16s
  - After release rule-host default: same command = 2.82s
  - After `--only-rule` narrowing and single-host ignore delegation: same command = 1.83s
  - Single-rule smoke: `--only-rule local/backend-json-tags` = 1.82s
