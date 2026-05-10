# Review and harden reusable metric signal changes

## Completed

- Reviewed the PR diff against `origin/main` with the local Rust and polint
  skills in mind.
- Changed `polint add-skill` so declining an overwrite keeps the existing skill
  without failing the command.
- Ensured `polint add-skill --all` continues to install missing skills when an
  existing skill is kept.
- Updated the tracked Claude skill so it matches the generated reusable metric
  signal guidance.
- Added focused SDK unit coverage for `FileMetrics`, `FunctionMetrics`, and
  `ComplexityMetrics` query helpers.

## Verification

- `cargo test -p polint add_skill --quiet`
- `cargo test -p polint metric_views_query_by_file_function_language_and_threshold --quiet`
- `cargo test -p polint external_rule_consumes_derived_metric_signals_through_public_sdk --quiet`
- `cargo fmt --all -- --check`
- `cargo test -p polint checked_in_examples_are_runnable_cli_fixtures --quiet`
- `cargo test -p polint-macros --quiet`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked --quiet`
- `cargo test -p polint --test cargo_install_smoke --locked -- --ignored`
- `target/debug/polint check --shortstat --no-cache --fail-on none` in
  `examples/code-quality-metrics`
- `target/debug/polint check --format json --only-rule local/code-quality-score --max-diagnostics 3 --no-cache --fail-on none`
  in `examples/code-quality-metrics`
- Generated Claude skill diffed cleanly against `.claude/skills/polint/SKILL.md`
- Manual `polint add-skill --all` smoke kept an existing Claude skill and
  installed the missing Codex skill.
