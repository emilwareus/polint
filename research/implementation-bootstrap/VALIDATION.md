# Validation

## What Was Validated In This Pass

Local source-code inspection:

- `Cargo.toml` workspace lint/dependency posture.
- `crates/polint/src/lib.rs` public/private boundary.
- `crates/polint/src/core/mod.rs` ID/fact/database/cache/capability/rule shape.
- `crates/polint/src/sdk/facts.rs` borrowed fact-view design.
- `crates/polint-macros/src/lib.rs` capability derivation.
- `crates/polint/src/analysis_plan.rs` plan/digest/support behavior.
- `crates/polint/src/cache/keys.rs` deterministic encoding.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs`
  parallel local parse/restore pattern.
- `crates/polint/src/module_graph/*` and `crates/polint/src/symbol_graph/*`
  builder/query/stable-key patterns.
- `crates/polint/tests/cli.rs` external-rule and unsupported capability tests.

Rust guidance:

- Local `.agents/skills/rust-best-practices` skill read and applied.
- Official Rust API Guidelines, Rust Book generics chapter, and Clippy docs
  consulted for API/dispatch/lint posture.

Prior research consistency:

- Checked against the roadmap sequence and the bootstrap sequence in
  `research/abstract-interpretation/implementation/BOOTSTRAP-SEQUENCE.md`.
- Recommendations align with analysis-kernel, evaluation-harness, call graph,
  data-flow, type/alias, effects/summaries, CFG, and extension-surface research.

## Validation Not Performed

- No product code was changed.
- No cargo tests were necessary for docs-only research.
- No benchmark was run.
- No external repository was cloned for this track.
- No subagents were spawned because the current user request did not explicitly
  ask for parallel agents in this turn, and current tool policy only allows
  spawning when explicitly requested.

## Required Validation Before Coding Is Considered Done

### Static Rust Checks

Run after implementation:

```text
cargo fmt --all
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all --locked
```

### Fact Determinism

Add tests for:

- stable keys identical across repeated runs;
- provider order stable across input ordering changes;
- facts sorted deterministically;
- diagnostics sorted deterministically.

### Cache Invalidation

Add regression tests for invalidation on:

- source hash change;
- `.polint.toml` lifecycle/config change;
- rule option change;
- analysis plan change;
- semantic schema change;
- provider version change;
- domain version/reduction graph change;
- extension manifest change;
- dependency summary digest change.

### Domain Laws

Use `proptest` for:

- join commutativity;
- join associativity;
- join idempotence;
- bottom/top behavior;
- transfer monotonicity where applicable;
- reduction soundness for P0 product domains.

### Extension Merge Gates

Test:

- accepted additive facts;
- rejected malformed facts;
- conflict when extension contradicts native exact fact;
- downgrade when extension supplies lower confidence;
- suppressive fact review diagnostics;
- provenance appears in debug output.

### Public SDK Promotion

Before exposing any new view:

- docs in `docs/facts/`;
- temp-repo rule test using only `polint::sdk::prelude::*`;
- capability diagnostics for unsupported/setup-missing cases;
- examples do not import internal modules;
- cache digest includes every behavior-affecting setting.

## Claim Scan

This report avoids claiming that the design is "complete" or globally optimal.
It is a first implementation path with high confidence from local code review
and prior research. The highest-risk areas remain stable-key memory cost,
summary invalidation shape, and how much extension lifecycle belongs in the
first implementation slice.
