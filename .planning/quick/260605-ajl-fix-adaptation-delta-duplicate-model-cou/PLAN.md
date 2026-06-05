# Fix adaptation delta duplicate model counts and held-out separation

## Scope

- Deduplicate accepted/rejected adaptation model fact counters across cases.
- Keep held-out cases out of top-level adapted delta counters when a held-out partition is supplied.
- Keep held-out cases visible in `cases` and summarized under `held_out`.

## Verification

- `cargo test -p polint eval::delta --locked`
