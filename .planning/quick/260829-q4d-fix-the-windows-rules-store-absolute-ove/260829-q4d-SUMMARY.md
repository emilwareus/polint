---
quick_id: 260829-q4d
status: complete
---

# Summary

Updated the rules-store root-resolution test to derive its absolute override
from `std::env::temp_dir()`, making the test valid on Windows without changing
`resolve_root` semantics. Neighboring override cases were checked; the other
platform-specific absolute paths are correctly guarded.

## Verification

- `cargo test -p polint --lib cache::rules_store` — 23 passed
- `cargo fmt --all -- --check` — passed
