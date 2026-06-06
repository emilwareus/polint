---
phase: 54-benchmark-promotion-gate-extension
artifact: final-audit
requirement: BENCH-01
status: passed-with-limitations
completed: 2026-06-06
---

# Phase 54 Final Audit: Benchmark Promotion Gate Extension

## Verdict

BENCH-01 is implemented and locally verified. The promotion gate now enforces
scoped precision floors, F0.5 alongside F1, separate per-language deltas,
false-positive trap flooding, the polyglot canary, determinism, and the
public-surface leak gate.

External Go x/tools and Jelly corpus final recall values are **limited/skipped**
in this local audit because the third-party benchmark clones and generated
result artifacts are intentionally not committed. This audit must not be used to
claim a measured Go/Jelly recall lift against those full external corpora.

## Final Verification Commands

| Command | Exit | Result |
|---|---:|---|
| `cargo test -p polint --lib eval::gates --locked` | 0 | passed, 9 tests |
| `cargo test -p polint polyglot --lib --locked` | 0 | passed, 3 tests |
| `cargo test --package polint --test public_surface_leak --locked` | 0 | passed, 5 tests |
| `cargo test -p polint --lib eval::determinism_gate --locked` | 0 | passed, 13 tests |
| `cargo test -p polint --locked` | 0 | passed: 2172 library tests, 144 CLI integration tests, 5 public-surface leak integration tests, 1 doctest; 1 slow smoke test ignored by default |
| `cargo clippy -p polint --all-targets --locked -- -D warnings` | 0 | passed |
| `cargo fmt --all -- --check` | 0 | passed |
| `git diff --check` | 0 | passed |

## Proof Matrix

| Proof | Verdict | Metric / value | Command or source | Notes |
|---|---|---|---|---|
| BENCH-01 requirement coverage | passed | Local gate/report/CI proof complete; external corpus measurements limited | `.planning/phases/54-benchmark-promotion-gate-extension/54-AUDIT.md` | Requirement is closed by enforcement, reporting, CI wiring, and explicit external-suite limitations. |
| Go precision floor | passed | `precision_floor.go.oracle-rta.setup_aware`; threshold `>= 0.6000`; tests cover `0.5900` fail and `0.6000` pass | `cargo test -p polint --lib eval::gates --locked`; `crates/polint/src/eval/gates.rs` | Enforces the hard Go precision floor independently from root metrics. |
| Jelly precision floor | passed | `precision_floor.typescript.oracle-jelly.setup_aware`; configurable threshold test uses `>= 0.5500` | `cargo test -p polint --lib eval::gates --locked`; `crates/polint/src/eval/gates.rs` | Verifies configurable Jelly-style precision floor enforcement. |
| F0.5 metric | passed | `f0_5 = 5.0 / 7.0` in metric test; scanner/report rows expose `F0.5` | `cargo test -p polint --locked`; `crates/polint/src/eval/metrics.rs`; `crates/polint/src/eval/markdown.rs` | F0.5 is precision-weighted and rendered in comparison and scanner sections. |
| F1 metric | passed | Existing `f1` remains in computed metrics, normalized reports, and markdown tables | `cargo test -p polint --locked`; `crates/polint/src/eval/metrics.rs`; `crates/polint/src/eval/markdown.rs` | Phase 54 adds F0.5 without removing F1. |
| Per-language deltas | passed | Go row passes recall delta `0.3000`; TypeScript/Jelly row with recall delta `0.1000` fails as expected; missing rows fail | `cargo test -p polint --lib eval::gates --locked`; `crates/polint/src/eval/gates.rs` | Deltas are scoped by language, suite/scoring mode, and precision tier. |
| False-positive flooding | passed | `false_positive_trap_hits = 1` fails against threshold `<= 0` | `cargo test -p polint --lib eval::gates --locked`; `crates/polint/src/eval/gates.rs` | Prevents precision from passing while trap fixtures flood false positives. |
| Polyglot Go+TS canary | passed | 3 canary tests selected by `polyglot` | `cargo test -p polint polyglot --lib --locked`; `.github/workflows/ci.yml` | CI promotion job runs this command on `ubuntu-latest` and `macos-latest`. |
| Public-surface leak gate | passed | 5 integration tests; prelude allowlist compiles and parser self-test detects synthetic leaks | `cargo test --package polint --test public_surface_leak --locked`; `.github/workflows/ci.yml`; `crates/polint/tests/public_surface_leak.rs` | v1.3 solver/eval internals remain outside `polint::sdk::prelude::*`. |
| Determinism gate | passed | 13 tests | `cargo test -p polint --lib eval::determinism_gate --locked`; `.github/workflows/ci.yml` | Promotion CI runs the same determinism command on Linux and macOS. |
| Runtime budget gates | passed | Existing runtime budget failures remain gate inputs | `cargo test -p polint --lib eval::gates --locked`; `crates/polint/src/eval/gates.rs` | Phase 54 did not add a new runtime threshold; the retained gate remains covered. |
| RSS reporting | limited | RSS is rendered in reports, but no Phase 54 hard RSS threshold was configured | `cargo test -p polint --locked`; `crates/polint/src/eval/markdown.rs`; `research/evaluation-harness/baselines/README.md` | RSS is reportable evidence, not a pass/fail promotion threshold in this local audit. |
| Cache quarantine gates | passed | Existing cache quarantine and determinism checks remain gate inputs | `cargo test -p polint --lib eval::gates --locked`; `cargo test -p polint --lib eval::determinism_gate --locked`; `crates/polint/src/eval/gates.rs` | Cache safety continues to be enforced through gate and determinism checks. |
| Go external x/tools recall final number | limited/skipped | Not measured in this local audit | `crates/polint/src/eval/external/go_x_tools_callgraph.rs`; `research/evaluation-harness/baselines/README.md` | Adapter tests confirm absent clones skip gracefully; no checked-in clone/output means no final recall claim. |
| Jelly external recall final number | limited/skipped | Not measured in this local audit | `crates/polint/src/eval/external/jelly_callgraph.rs`; `research/evaluation-harness/baselines/README.md` | Adapter tests confirm absent clones skip gracefully; no checked-in clone/output means no final recall claim. |
| CI promotion gate wiring | passed | `promotion-gate` matrix covers `ubuntu-latest` and `macos-latest` | `.github/workflows/ci.yml` | Runs polyglot, public-surface leak, and determinism commands together with fail-fast disabled. |

## Limitations

- The audit records mechanism-level precision-floor and per-language delta
  enforcement, not a reproduced full external Go/Jelly corpus result.
- Full external Go x/tools and Jelly benchmark clones are intentionally excluded
  by the baseline artifact policy, so their final recall values remain
  unavailable in this local closeout.
- RSS is present in report output, but Phase 54 did not configure a hard RSS
  promotion threshold.
