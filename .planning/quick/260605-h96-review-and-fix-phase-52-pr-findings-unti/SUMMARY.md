# Summary

Fixed Phase 52 PR review findings and repeated focused subagent review until two consecutive rounds returned no findings.

## Fixes

- Filtered public unknown rows so external imports are not reported as missing facts.
- Avoided duplicating Go package-load/provider-quality diagnostics in the unknown taxonomy.
- Redacted internal provider, fact-family, and source stable-key details from public unknowns JSON.
- Closed the unknowns JSON schema objects with `additionalProperties: false` and added schema regression coverage.
- Made refined-call derivation additive again across direct, framework, Go, TS/JS, summary, extension, and solver-assisted edges.
- Projected solver-derived Go RTA call edges through semantic call-constraint keys and raw Go semantic callsite keys.
- Included solver output in the refined-call provider digest.
- Added a persisted Go RTA fixture that asserts an exact `runDispatch -> Dog.Speak` `PointsToAssisted` refined edge.
- Restored positive extension-model refined-call expectations.
- Mirrored Go RTA's zero-width method-caller mapping in refined-call projection and added a decoy same-span regression test.

## Verification

- `cargo test -p polint solver_projection -- --nocapture`
- `cargo test -p polint refined_calls_direct_vs_refined_fixture_projects_persisted_rta_edge -- --nocapture`
- `cargo test -p polint eval_native_fixture_runner_refined_calls_fixture_passes -- --nocapture`
- `cargo test -p polint eval_refined_calls_manifests_cover_required_taxonomy -- --nocapture`
- `cargo test -p polint unknowns -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo clippy -p polint --all-targets -- -D warnings`

## Review Loop

- Round 1 found unknowns public-contract issues and solver projection gaps; fixed.
- Round 2 found the zero-width Go semantic method-caller projection mismatch; fixed.
- Round 3 returned `NO FINDINGS` from unknowns/CLI/schema and refined-call/projection reviewers.
- Round 4 returned `NO FINDINGS` from unknowns/CLI/schema and refined-call/projection reviewers.

Outcome: two consecutive clean review rounds achieved.
