# Quick Task 260512-aop: Fix review findings for baseline and module relationships - Summary

**Date:** 2026-05-12
**Status:** Complete
**Code Commit:** `30098c6`

## What Changed

- Fixed TS path-alias misses so configured `paths` aliases that fail to resolve remain `Unresolved(NotFound)` instead of becoming external package dependencies.
- Added a small tsconfig path-alias index for the `NotFound` resolver path, because Oxc can report alias target misses as ordinary not-found errors.
- Reworked baseline matching to use a baseline-specific, path-independent identity for default diagnostics while keeping exact stable-fingerprint compatibility.
- Hardened baseline identity for repeated default diagnostics in the same file by assigning deterministic occurrence fingerprints during batch classification and update.
- Made central baseline `ignore` entries file-specific so a suppression for one path cannot hide or remove an unrelated baseline entry with the same default fingerprint.
- Kept baseline relocation conservative: moved diagnostics are refreshed only when the old entry and current diagnostic are one-to-one; ambiguous duplicate moves stay new/fixed instead of being silently suppressed.
- Extended TS array-element traversal through Oxc's inherited expression variants, covering `import("./lazy")` and dynamic `import(name)` inside array literals.
- Fixed setup-missing capability reasons so TS-only resolver failures report a TS/JS setup reason instead of the default Go metadata reason.
- Made the TS path-alias fallback read commented tsconfig JSON and inherited relative/package `extends` configs when deciding whether a missing non-relative import is an unresolved internal alias.
- Updated baseline documentation and generated skill text to describe moved-path baseline matching and file-specific central ignores.

## Tests Added

- Unit coverage for unambiguous and ambiguous baseline relocation.
- Unit and CLI coverage for repeated default diagnostics in one file.
- Unit coverage proving central `ignore` entries do not remove unrelated baseline entries with the same default fingerprint.
- CLI coverage for moving a default diagnostic after `polint baseline create`.
- Module graph coverage for missing TS path aliases, commented/extended tsconfig aliases, and TS-only setup diagnostics.
- TS parser coverage for dynamic imports inside array elements.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test -p polint baseline::tests --locked`
- `cargo test -p polint module_graph_ts_resolution_keeps_missing_alias --locked`
- `cargo test -p polint module_graph::tests --locked`
- `cargo test -p polint ts::tests --locked`
- `cargo test -p polint baseline_create_preserves_repeated_default_diagnostics_in_same_file --locked`
- `cargo test -p polint baseline_update_refreshes_unambiguous_moved_default_diagnostic_paths --locked`
- `cargo test --workspace --all-features --locked`
