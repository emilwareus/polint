# Quick Task 260512-h4g Summary

## Outcome

Hardened `scripts/publish-crates.sh` so crates.io publishes are resumable after a partial success.

## Changes

- Replaced exact-version detection based on `cargo search` with `cargo info <crate>@<version> --registry crates-io` from a temporary directory, avoiding search endpoint lag and local workspace package shadowing.
- Added `publish_crate`, which skips exact versions that already exist on crates.io.
- Kept post-publish waiting, but now checks Cargo registry metadata instead of crates.io search output.
- Switched publish authentication from deprecated `cargo publish --token` to `CARGO_REGISTRY_TOKEN`, while still accepting `CRATES_IO_TOKEN` from CI.

## Verification

- `bash -n scripts/publish-crates.sh`
- `git diff --check`
- `DRY_RUN=1 ./scripts/publish-crates.sh`
- Confirmed registry state:
  - `polint-macros@0.1.10` exists
  - `polint@0.1.10` does not exist

## Commit

- `0729a6b` - `fix: make crate publish script resumable`
