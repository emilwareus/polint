# Summary: Fix Windows Platform Library CI

## Completed

- Replaced the Unix-only absolute path literal in
  `workspace_join_rejects_escape_paths_and_symlinks` with a real `tempdir()`
  absolute path.
- This keeps the test assertion portable across Windows and Unix.

## Verification

- `cargo test -p polint eval::runner::tests::workspace_join_rejects_escape_paths_and_symlinks --locked -- --nocapture` - passed.
- `cargo test -p polint --lib eval::runner --locked` - passed, 8 tests.
- `cargo fmt --all --check` - passed.
