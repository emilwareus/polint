# Quick Task 260521-m9k: Fix critical PR review findings

## Scope

Fixed critical review findings in the direct-call and abstract-domain work before updating PR 35.

## Changes

- Prevented unsupported call evidence from attaching to unrelated call sites in the same file when there is no operation or span match.
- Made abstract-domain reads of previously unseen places conservative by recording maybe-uninitialized state instead of initialized state.
- Made branch refinement preserve contradictions by marking impossible branch states unreachable instead of overwriting prior facts.
- Replaced dense place IDs in abstract-domain observation stable keys and output digests with stable place keys in provider-backed output.
- Cleaned strict clippy failures surfaced by the full check pass.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --locked`
