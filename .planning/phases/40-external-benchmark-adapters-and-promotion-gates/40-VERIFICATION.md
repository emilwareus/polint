# Phase 40 Verification: External Benchmark Adapters And Promotion Gates

Completed: 2026-05-26

## Verdict

PASS. Phase 40 delivered internal external-benchmark adapters, comparison rows,
polint baseline/adapted run records, adaptation prompt artifacts, deterministic
tier selection, native promotion gates, baseline regression gates, and public
boundary proof for SAE-PROM-01.

## What Is Measured

- Native promotion fixtures measure graph/fact/path matching, unknown budgets,
  runtime budgets, cache determinism, and promotion gate verdicts.
- Supported-language smoke suite manifests exist for:
  - SecBench.js smoke at source commit `bc3156219138`, language support
    `supported`, with deterministic fast/nightly/release tiers.
  - gosec samples at source commit `de65614d10a6`, language support
    `supported`, with deterministic fast/nightly/release tiers.
- Unsupported-language suite manifests and adapters have been removed from the
  active benchmark scope. Current external benchmark manifests cover only Go and
  TypeScript/JavaScript.
- Adapted benchmark reports must include the exact adaptation prompt path/hash,
  declared budget, allowed/forbidden inputs, changed artifacts or no-change
  reason, changed rule/extension digests, and default-vs-adapted deltas.

## Public Claim Boundary

Phase 40 does not promote eval as a stable CLI, SDK, runner API, public JSON
schema, or docs/facts surface. `docs/API-VISIBILITY-PLAN.md` records that public
query-view and eval/report promotion remains deferred to Phase 41.

## Verification Commands

- `cargo fmt --all --check` - passed
- `cargo clippy --workspace --all-targets -- -D warnings` - passed
- `cargo test --workspace --all-targets --locked` - passed
- `cargo test -p polint --lib eval::runner --locked` - passed, 8 tests
- `cargo test -p polint --lib eval --locked` - passed, 193 tests
- `cargo run -q -p polint -- --help > /tmp/polint-help.txt && ! rg -n "\\beval\\b" /tmp/polint-help.txt` - passed
- `rg -n "polint eval|CallGraph<'_|DataFlow<'_|Evidence<'_" README.md docs/facts crates/polint/src/sdk crates/polint/src/runner || true` - only found the existing `docs/facts/data-flow.md` note that `DataFlow<'_>` is reserved and unsupported.

## Evidence Files

- `crates/polint/src/eval/suite.rs` - suite manifest, support labels, tier metadata.
- `crates/polint/src/eval/adapter.rs` and `crates/polint/src/eval/external/*` - external adapter shapes and scorer/parser logic.
- `crates/polint/src/eval/runner.rs` and `crates/polint/src/eval/tiers.rs` - internal run planning, deterministic selection, path safety, hidden test report helper.
- `crates/polint/src/eval/gates.rs` and `crates/polint/src/eval/baseline.rs` - promotion and baseline regression gates.
- `crates/polint/src/eval/adaptation.rs` and `crates/polint/src/eval/delta.rs` - adaptation records and default-vs-adapted deltas.
- `research/evaluation-harness/prompts/default-adaptation-agent.md` - exact default adaptation-agent prompt.
- `research/evaluation-harness/suites/*.toml` - pinned external suite manifests.
- `research/evaluation-harness/baselines/README.md` - committed baseline artifact policy.
- `tests/eval-fixtures/promotion/cfg-call-flow-evidence/` - native promotion fixture.
- `tests/eval-fixtures/extension/adaptation-delta/` - synthetic adaptation-delta fixture.

## Limitations

- Full external benchmark corpora are not vendored and must remain under
  gitignored local clones.
- Benchmarks for languages without a polint frontend are excluded until a future
  language-adapter phase adds that support.
- The internal eval schema is intentionally unstable. Phase 41 owns any public
  SDK/query or CLI promotion.
