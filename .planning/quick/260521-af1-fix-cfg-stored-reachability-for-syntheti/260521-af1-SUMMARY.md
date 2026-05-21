# Quick Task 260521-af1 Summary: Fix CFG Stored Reachability For Synthetic Exits

## Outcome

Fixed the follow-up PR review finding where CFG blocks created for optional synthetic exits could remain stored as reachable even when no normal-control path reached them.

## Changes

- Recomputed stored CFG block reachability in `CfgBuilder::finish` from each function entry over `NormalControl` edges before normalization.
- Added a builder regression test proving an unconnected synthetic exceptional exit is stored as unreachable.
- Added Go and TS/JS abrupt-only regression assertions that reachable exceptional exits do not leave stale reachable flags on untargeted normal exits.

## Verification

- `cargo test -p polint --lib analysis::cfg::builder --locked`
- `cargo test -p polint --lib analysis::cfg::lower_go --locked`
- `cargo test -p polint --lib analysis::cfg::lower_ts --locked`
- `cargo test -p polint --lib analysis::cfg --locked`
- `cargo test -p polint --lib analysis_kernel::validation::cfg --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
