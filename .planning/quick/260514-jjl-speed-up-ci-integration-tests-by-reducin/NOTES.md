# Quick Task 260514-jjl

Speed up CI integration tests by reducing repeated nested Cargo builds.

Status: implemented and verified.

Root cause:
- `tests/cli.rs` exercises repo-local rule packs through `polint check`.
- `polint check` launches nested `cargo run` for each temp `.polint/rules` crate.
- Those temp crates were building into separate temp `target/` directories, so CI repeatedly compiled the same dependency graph. Windows amplified this into ~32 minutes for the CLI test binary.

Fix:
- Test-spawned `polint` and nested Cargo commands now inherit `CARGO_TARGET_DIR=target/polint-cli-test-cargo`.
- Temp rule-pack manifests get path-derived unique package names so parallel temp crates can safely share the target directory without binary-name collisions.

Verification:
- `cargo fmt --all`
- `cargo test -p polint --test cli -- --nocapture`: 97 passed, 81.74s with a clean shared nested target.
- `cargo test --workspace --all-features --locked`: passed, 79.80s with a clean shared nested target.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test -p polint --test cargo_install_smoke --locked -- --ignored`: passed, 21.95s.
