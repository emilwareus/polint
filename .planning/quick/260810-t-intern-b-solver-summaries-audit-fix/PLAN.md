# Quick Plan: T-INTERN-B solver/summaries audit-fix

## Goal
Fix adversarial-review findings on 0c6dc64d..e5057d7d: no StableKeyId Ord/BTree ordering for hop/path keys; normalize TS points-to before return; remove numeric StableKeyId serde from identity facts; strengthen reverse-allocation determinism tests; harden Go RTA membership maps.

## Out of scope
- Solver densification
- Dual identity fields / Debug rewrite shims
- FactMeta (T-INTERN-C)
