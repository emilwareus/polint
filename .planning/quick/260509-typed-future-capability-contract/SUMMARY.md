# Summary

Implemented the typed future capability contract cleanup.

## Changed

- Added reserved `DataFlow<'_>` / `dataflow` capability wiring.
- Centralized requested capability names through `Capabilities::requested_names`.
- Changed rule execution so rules with unsupported or setup-missing hard capabilities emit plan diagnostics and do not execute with placeholder facts.
- Added setup-missing diagnostic coverage and unsupported-capability skip coverage.
- Updated roadmap, research docs, fact docs, and `AGENTS.md` so future CFG, call graph, dataflow, coverage, module graph, symbols, references, and test metrics are described as typed SDK views instead of `RuleCtx` fact helpers.

## Verified

- `cargo test -p polint analysis_plan --locked`
- `cargo test -p polint blocking_capabilities --locked`
- `cargo test -p polint-macros --locked`
- `./scripts/release-local-check.sh`
- `cargo install --path crates/polint --locked --force`
- installed `polint --version`
- installed `polint check --format json --fail-on none` across all `examples/*/.polint.toml`
- generated temp project smoke with `polint init`, `polint new-rule ts no-inline-colors`, `polint explain plan --format json`, and `polint check --format json --fail-on none`
