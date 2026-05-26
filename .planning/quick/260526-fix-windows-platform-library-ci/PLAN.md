# Quick Task: Fix Windows Platform Library CI

## Goal

Fix the Windows-only CI failure in
`eval::runner::tests::workspace_join_rejects_escape_paths_and_symlinks`.

## Diagnosis

The test used `Path::new("/tmp/outside")` as the absolute-path case. That is a
Unix absolute path, but it is not a reliable absolute path on Windows.

## Plan

- Replace the hard-coded Unix absolute path with an actual `tempdir()` path.
- Run the focused test and formatting checks.
- Commit and push the CI fix.
