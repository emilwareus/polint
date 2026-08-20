# Quick Plan: T-INTERN-B rest-small-facts

## Goal
Migrate remaining identity-owning analysis fact families from `stable_key: String` to `StableKeyId` with no dual paths:
identity, extensions, adaptation, types, values, points_to, aliases, access_paths, reachability, refined_calls.

## Constraints
- No FactMeta / stable_key_owners (T-INTERN-C)
- No solver densification
- Resolve text at digest/sort/diagnostic/debug boundaries; never sort by allocation order
- Debug output text fields must not remain literal `stable_key: String`
- Public API paths unchanged
- Diagnostic goldens byte-identical

## Validation
cargo check/clippy/fmt + public_surface_leak + determinism_gate; golden once if green
