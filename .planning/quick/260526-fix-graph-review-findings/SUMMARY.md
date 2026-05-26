# Summary: Fix Graph Review Findings

## Completed

- Added regression coverage for unresolved dynamic member calls so they cannot
  resolve through the name-only lexical fallback.
- Kept the lexical fallback available for unresolved static member syntax.
- Added regression coverage for Go x/tools txtar case ids that sanitize to the
  same directory name.
- Changed txtar materialization to append a stable case-id hash to the scratch
  directory name.

## Verification

- `cargo test -p polint analysis::calls::direct --locked -- --nocapture` - passed.
- `cargo test -p polint eval::external::go_x_tools_callgraph --locked -- --nocapture` - passed.
- `cargo test -p polint --lib eval --locked` - passed, 206 tests.
- `cargo clippy -p polint --all-targets --locked -- -D warnings` - passed.
- `cargo fmt --all --check` - passed.
