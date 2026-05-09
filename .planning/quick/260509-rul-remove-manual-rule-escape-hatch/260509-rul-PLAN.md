# Quick Task 260509-rul: Remove manual Rule implementation escape hatch

**Date:** 2026-05-09
**Mode:** quick local execution

## Goal

Fail forward during beta: remove the public/manual `impl Rule` authoring shape
so normal repo-local rules cannot preserve handwritten capabilities by habit.
The released shape should be macro-derived and analyzable.

## Tasks

- [x] Replace the public `Rule` trait API with an opaque rule value constructed by
   the macro/runtime.
- [x] Update `#[polint::rule]`, runner, analysis-plan, cache, and tests to use the
   opaque value instead of `Arc<dyn Rule>`.
- [x] Rewrite internal tests that relied on manual trait implementations to use
   explicit internal constructors or macro-generated rules.
- [x] Update docs and AGENTS.md to make the no-backwards-compatibility posture
   explicit while the project is in beta.
- [x] Run full verification and push the PR branch.

## Verification

- `cargo fmt --all -- --check`
- `cargo check --workspace --locked`
- `cargo test --workspace --all-features --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- Installed CLI and example smoke tests if runtime paths changed.
