---
status: complete
task: refuse cross-checkout rule-host sharing when Rust sources can embed Cargo-provided checkout paths
implementation_commit: this commit
---

# Quick Task 260829-s1l Summary

Added a conservative byte-token scan over Cargo's conventional Rust source
trees and root `build.rs`. Packages mentioning `CARGO_MANIFEST_DIR`, `OUT_DIR`,
or the path-bearing `CARGO`/`RUSTC` environment names are local-only for both
store restore and publication; `CARGO_PKG_*` remains shareable. The fingerprint
now records the gate state.

Updated the shared-store README contract and changed the release review verdict
to ready. The intentionally broad comment false-positive, token-free package,
normal source, build-script, executable-name, and package-metadata cases are
covered by unit tests.

All requested gates pass: formatting, workspace Clippy with warnings denied,
32 focused cache unit tests, and the end-to-end rule-host-store integration
test. No push was performed.
