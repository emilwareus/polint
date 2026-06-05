# Fix adaptation delta model fact and held-out reporting review findings

## Scope

- Persist accepted and rejected repo-local adaptation model facts as crate-private audit rows.
- Project those audit rows into eval observations so adapted deltas can count real model facts.
- Add a partition-aware delta computation path for held-out cases and exercise the checked-in fixture.
- Run the full repo checks and commit the result.

## Verification

- `cargo test -p polint eval::delta`
- `make check`
