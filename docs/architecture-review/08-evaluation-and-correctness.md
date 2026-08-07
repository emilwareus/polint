# 08 — Evaluation, Correctness Assurance, and Testing Architecture

**Scope:** `crates/polint/src/eval/` (29,344 LOC / 35 files), `tests/eval-fixtures/`,
`tests/fixtures/`, `crates/polint/tests/`, `.github/workflows/`,
`research/evaluation-harness/`, `performance/*.md`.

**Verdict up front.** polint has *the skeleton of a serious evaluation program* —
a suite-manifest schema, tiering, a matcher, a metrics module with F0.5/F1/F2/F3,
promotion gates, a determinism gate, and adapters for two real external call-graph
oracles. It also has a genuinely excellent set of mechanically-enforced
architectural invariants. But **almost none of the accuracy machinery is wired to
anything that can fail.** The headline capability number (Jelly F1 88.75%) exists
only in a markdown file; the committed baseline JSON that is supposed to hold it
contains `null`; the test that would produce it silently `return`s on CI; and no
test anywhere in the repository asserts a precision, recall, or F1 value.

The claim "world's most capable static analysis engine" is currently
**unfalsifiable by the repo's own CI**.

---

## (a) What is measured today — with real numbers

### a.1 The eval harness is an internal test tool, not a product

| Property | Evidence |
|---|---|
| Not a CLI subcommand | `crates/polint/src/cli/mod.rs:161-190` — `enum Command` is `Init, AddSkill, NewRule, Baseline, Cache, Inspect, Facts, Unknowns, Explain, Test, Check, Review, Ignores`. No `Eval`, hidden or otherwise. |
| Crate-private | `crates/polint/src/lib.rs:26` — `pub(crate) mod eval;` |
| Dead code in release builds | `crates/polint/src/eval/mod.rs:1-7` — `#![cfg_attr(not(test), expect(dead_code, reason = " defines the internal eval schema before later harness plans consume it"))]` |
| `observed.rs` is 100% test-only | Every item in the 4,027-line file carries `#[cfg(test)]`, starting at `crates/polint/src/eval/observed.rs:1` |
| Actively kept out of the public surface | `crates/polint/tests/public_surface_leak.rs:469` forbids `polint::eval`; `crates/polint/src/eval/runner.rs:1004` asserts the string `"polint eval"` appears in no README/SDK/docs text |
| No feature flag | `crates/polint/Cargo.toml` `[features]` = `default = []`, `bench = []` (the latter is for `polint-bench` only) |

So: 29,344 LOC — **11% of the 267,710-line Rust codebase** — compiles to nothing
in a shipped binary. That is defensible for a test harness, but it means the
"eval" investment is not a differentiator users can run.

### a.2 The native fixture corpus is tiny

| Metric | Value |
|---|---|
| Fixture cases (`expected.polint-eval.toml`) | **53** |
| Total files under `tests/eval-fixtures/` | 253 |
| **Total analyzed source LOC across all fixtures** | **1,658** (`tests/eval-fixtures/`), **1,936** including `tests/fixtures/` |
| Largest single fixture source file | 58 LOC (`tests/eval-fixtures/jelly/ts-inventory-spans/repo/src/forms.ts`) |
| `examples/` source (also used as test input) | 62 files, 249 LOC |
| Expected assertion rows | **381** |

Breakdown of the 381 expected rows by kind:

| Kind | Count | What it actually asserts |
|---|---:|---|
| `invariant` | 223 | An internal name/value string, e.g. `data_flow.counts.local_edges.nonzero = true`. **Excluded from precision/recall** (`crates/polint/src/eval/metrics.rs:277-285`). |
| `fact` | 135 | A `(family, stable_key, precision, status)` tuple exists |
| `path` | 57 | An evidence path exists |
| `runtime_budget` | 26 | Case ran under N ms |
| `graph_edge` | **7** | A call/graph edge exists |
| `diagnostic` | **6** | A rule fired at a location |

Assertion-mode distribution across all fixtures: **287 `exact`, 108 `partial`,
3 `tolerant`, and 0 `forbidden`**. `AssertionMode::Forbidden` exists in the schema
(`crates/polint/src/eval/model.rs:134-139`) and **is used by zero fixtures**.

Coverage per analysis is thin to the point of being nominal:

| Analysis area | Fixture cases |
|---|---:|
| `go-rta` | 7 |
| `determinism` | 5 |
| `semantic-graph`, `identity` | 4 each |
| `type-value-alias`, `ts-object-model`, `extension`, `cache` | 3 each |
| `ts-tokens`, `refined-calls`, `provenance`, `direct-summaries` | 2 each |
| **`data-flow`, `cfg-core`, `direct-calls`, `evidence`, `module-topology`, `jelly`, `framework-entrypoints`, `semantic-mir`, `semantic-index`, `abstract-domains`, `kernel`, `promotion`, `polyglot-canary`** | **1 each** |

### a.3 The single data-flow fixture is the whole taint story

`tests/eval-fixtures/data-flow/core/` is the entire on-disk ground truth for
data flow. Its program (`repo/src/app.ts`) is 32 lines. Its assertions are four
`mode = "partial"` "an edge of this shape exists" rows plus two
`*.nonzero = true` invariants and a runtime budget.

There is **no source→sink taint ground truth, no sanitizer-correctness
assertion, and nothing that could catch a missed flow.** The underlying
`crates/polint/src/analysis/data_flow/` module is 5,275 LOC with 41 in-module
unit tests.

### a.4 The security rule templates are validated tautologically

polint ships 10 flagship policy templates
(`crates/polint/src/cli/mod.rs:207-222`): SSRF, dangerous-html,
unsafe-deserialization, secret-to-log, request-to-shell, pii-to-analytics,
user-file-path, sensitive-write-guard, transaction-cleanup, raw-reachable-api.

`crates/polint/tests/cli.rs:7021-7062` scaffolds all 10 and runs `polint test`,
asserting `summary.total == 20, failed == 0` — one positive and one negative
fixture per template. That is a real end-to-end check, and it is the *only*
security evaluation in the repository.

But the fixtures are generated from the same source as the rule. The SSRF
"clean" fixture (`crates/polint/src/cli/mod.rs:1753-1763`):

```ts
function allowlist_url(url: string): string { return url; }
function fetchUrl(url: string) {}
app.get("/fetch", function handler(req, res) {
  fetchUrl(allowlist_url(String(req.query.url)));
});
```

The barrier list in the generated rule is literally
`["allowlist_url", "validate_url"]` (`crates/polint/src/cli/mod.rs:1429`). The
fixture passes because a name matches a name. This measures *plumbing*, not
*analysis*. 20 hand-matched data points across 10 vulnerability classes is not
an evaluation of a taint engine.

### a.5 External benchmarks: real numbers, but they live in markdown

**Jelly (JS/TS call graph).** Ground truth = the `*.json` call-graph oracles
inside a clone of `cs-au-dk/jelly` @ `b799ed4f0d68c670fe398830aaa51dd5c628cf74`.
**76 cases, 1,479 expected edges.** The clone path
(`research/evaluation-harness/repos/jelly`) is gitignored (`.gitignore:31`).

Trajectory (all from `performance/*.md`, none from code):

| Date | TP / FP / FN | Precision | Recall | F1 | Source |
|---|---|---:|---:|---:|---|
| 2026-06-06 | 8 / 6 / 1471 | 57.14% | 0.54% | 1.07% | `performance/2026-06-06-static-analysis-performance.md:20` |
| 2026-06-14 | 1076 / 44 / 403 | 96.07% | 72.75% | 82.80% | `performance/2026-06-14-f1-recall-research.md:5` |
| 2026-06-15 | 1152 / 47 / 327 | 96.08% | 77.89% | 86.03% | `performance/2026-06-15-jelly-fn-decomposition.md:62-65` |
| **2026-06-17 (latest)** | **1219 / 49 / 260** | **96.14%** | **82.42%** | **88.75%** | `performance/2026-06-17-jelly-fn-categorization-and-wins.md:3-4` |

That is a genuinely impressive 11-day trajectory and the engineering behind it
is real. Two caveats:

1. **The score is not pure oracle agreement.** `jelly_reachable_caller_spans`
   (`crates/polint/src/eval/external/jelly_callgraph.rs:237-289`) prunes observed
   edges whose caller is unreachable from a polint-defined root set. Unpruned the
   same run is **1295 / 1137 / 184 → P ≈ 53.2%, F1 ≈ 66.2%**
   (`performance/2026-06-17-jelly-fn-categorization-and-wins.md:28`). The pruner
   is a polint-authored normalization, not something Jelly specifies. An outsider
   diffing against a raw Jelly run gets a very different number.
2. **The headline requires `POLINT_GRAPH_BENCH_TIER=release`.** The default tier
   is `fast`, which is `sample:balanced:20`, i.e. 20 of 76 cases
   (`research/evaluation-harness/suites/jelly-callgraph-micro.toml:29-33`,
   `crates/polint/src/eval/external/mod.rs:93`).

**Go x/tools RTA.** 5 `.txtar` cases, 37 expected edges, last measured
2026-06-06 at **37 TP / 6 FP / 0 FN, P 86.05%, R 100.00%, F1 92.50%**
(`performance/2026-06-06-static-analysis-performance.md:19`). The same doc
(lines 53-56) honestly notes the FPs are mostly real edges absent from partial
`WANT:` comments — *"not a closed negative oracle"*. **This number has not been
refreshed in 7 weeks** despite continuous engine change.

**gosec and SecBench.js measure nothing.** Both adapters are enumeration stubs
with constant placeholder labels:
- `crates/polint/src/eval/external/gosec.rs:132-139` assigns every case
  `rule_id: "gosec/sample-native-label"` with `line: None, fingerprint: None`.
  Case discovery is a filename heuristic — any `.go` path containing
  `test`/`sample`/`benchmark`/`rule` (`gosec.rs:119-124`).
- `crates/polint/src/eval/external/secbench_js.rs:130-137` assigns
  `rule_id: "secbench-js/unlabelled-executable-case"`. The id says
  *unlabelled*. Its own limitation string (`secbench_js.rs:88-91`) admits the
  exploit tests are never executed, so there is no oracle at all.
- Neither implements `normalize_kernel_output`, so neither can score a polint
  run even in principle.

`research/evaluation-harness/FINAL-REPORT.md:41-42` lists both as active
"Current Supported Suites". They are not.

### a.6 What actually runs and can fail in CI

CI (`.github/workflows/ci.yml`, 13 jobs) completed the last run in **~9m53s
wall clock** (2026-07-20, `14:53:46Z → 15:03:39Z`); the longest job is
`windows-latest — lib tests` at 9m52s. Jobs:

| Job | What it enforces |
|---|---|
| `rustfmt`, `clippy -D warnings`, `rustdoc -D warnings`, `cargo deny`, MSRV 1.95 | Hygiene / supply chain |
| `test-linux` (matrix: `--lib` and `--test '*'`) | The 2,616 tests |
| `test-platform` (windows, macos, `--lib`) | Platform parity |
| `gates` (ubuntu + macos) | polyglot canary; public-surface leak; **determinism gate N=10**; semantic-store RSS/wall-clock boundary |
| `install-smoke`, `sarif` | `cargo install` works; SARIF 2.1.0 shape |

**There is no accuracy gate.** The only committed numeric regression baseline
that CI enforces is *performance*:
`research/evaluation-harness/baselines/store-disabled-check.json` —
peak RSS 46,645,248 bytes, cold 26 ms, warm 22 ms, on a fixture literally named
`polint-tiny-fixture`, with budgets +20% RSS / +25% cold wall clock
(`crates/polint/src/eval/baseline.rs:199,205`).

---

## (b) What is NOT measured — and why each gap is dangerous

### b.1 No test asserts any precision, recall, or F1 value. Anywhere.

The single end-to-end external-benchmark test
(`crates/polint/src/eval/external/mod.rs:18-89`) asserts only:

```rust
assert!(!jelly.cases.is_empty());
assert!(jelly.metrics.graph_edges_expected > 0);
assert!(!jelly.output_hash.is_empty());
assert!(jelly.comparison_rows.len() >= 2);
```

`run.metrics.precision` and `.recall` are computed, formatted into a markdown
table (`external/mod.rs:117-129`), and **never compared to anything**.

**Danger:** F1 can regress from 88.75% to 40% and every job stays green.

### b.2 The benchmark silently no-ops on CI

`crates/polint/src/eval/external/mod.rs:27-29`:

```rust
if !go_repo.exists() || !jelly_repo.exists() {
    return;
}
```

Same pattern at `jelly_callgraph.rs:312-320` and `jelly_callgraph.rs:1521-1524`,
`go_x_tools_callgraph.rs:194-201`, `gosec.rs:69-77`, `secbench_js.rs:69-77`.
Since `.gitignore:31` excludes `research/*/repos/`, **this branch is taken on
every CI run and every fresh developer checkout.** It is a bare `return`, not
`#[ignore]`, not a `println!`, not a skip marker. A green run is
indistinguishable from a run that measured nothing.

### b.3 The committed accuracy baseline is empty

`research/evaluation-harness/baselines/persisted-graph-accuracy.json`:

```json
{ "suite_id": "jelly-callgraph-micro", "recall": null, "precision": null,
  "graph_edges_expected": 0, "graph_edges_observed": 0, "unknown_count": 0 }
```

Both rows are null. Requirement **BENCH-04** ("Persisted-graph recall/precision
is measured … and surfaced in benchmark reports, giving the next milestone's
accuracy work a starting point") is marked `[x]` complete in
`.planning/REQUIREMENTS.md:68`. The artifact it produced records nothing. The
repo's own `baselines/README.md:51-54` says so explicitly:

> *"Do not use the local Phase 54 audit alone to claim a measured recall lift
> against those full external suites."*

That honesty is admirable and the requirement should not have been checked off.

### b.4 The promotion-gate machinery has no production caller

`gates.rs` is 746 lines implementing `min_native_pass_rate`, `precision_floors`,
`required_per_language_deltas`, `max_graph_edge_misses`, unknown budgets, and a
determinism check. `evaluate_promotion_gates` is called from exactly two places
(`crates/polint/src/eval/runner.rs:963-964`), **both inside
`#[cfg(test)] mod tests`**, and both against the *synthetic* promotion fixture
that never runs the engine.

`PromotionGateThresholds::default()` (`gates.rs:34-48`) sets
`precision_floors: Vec::new()` and `required_per_language_deltas: Vec::new()` —
**no precision floor is enforced by default**, and no suite manifest under
`research/evaluation-harness/suites/*.toml` configures one. The `0.60` / `0.55` /
`0.25` numbers at `gates.rs:476,507,537,567` are test-local literals.

### b.5 Native fixtures assert recall-only; precision is never checked

Every native-fixture test asserts `false_negatives == 0` plus `forbidden_hits == 0`
plus `runtime_budget_failed == 0` — see `fixtures.rs:1566,1731,1809,1892,1959,
2071,2115,2258` and `runner.rs:726,746,941`, ~30 sites. **Zero assertions on
`false_positives`, `precision`, `f1`, or `recall`.**

This matters because `matcher.rs:334-342` classifies *every unmatched observed
row* as a false positive. A real fixture run emits thousands of observed facts
against 10-40 expected rows, so `false_positives` is structurally enormous and
is simply never read. Combined with **zero `mode = "forbidden"` fixtures** and
exactly **one** `false_positive_trap` (in a synthetic fixture), the harness has
essentially **no negative controls**. An engine change that doubles spurious
edges passes everything.

### b.6 Three fixtures are pure tautologies

`fixtures.rs:161-165`:

```rust
let observed = if fixture.manifest.synthetic_observed {
    fixture.manifest.observed.clone()
} else {
    observe_kernel_fixture(&fixture)?
};
```

`tests/eval-fixtures/promotion/cfg-call-flow-evidence/`,
`extension/adaptation-delta/`, and `extension/rejection-delta/` set
`synthetic_observed = true` and hand-write both sides into the same TOML. The
engine is never invoked. (Credit where due: `fixtures.rs:109-125` restricts this
to three areas, and `fixtures.rs:1616-1618` explicitly asserts the evidence
fixture is *not* synthetic — the codebase is self-aware about the risk. The
general "is observed a recorded snapshot?" worry is **unfounded**: for the other
50 fixtures, observed rows are computed live from `AnalysisKernel::run` at
`observed.rs:404-411`.)

### b.7 Zero fuzzing, zero differential testing

- **No `cargo-fuzz`, no `fuzz/` directory, no `libfuzzer`, no `arbitrary`,
  no `afl`.** Confirmed by dependency and directory search.
- **No differential testing** against a second implementation or against another
  tool. `competitors.rs` (443 LOC) is a *schema* for comparison rows
  (`BenchmarkComparisonRow`, `ProductIdentity`, `ResultSource`) with validation —
  and contains no actual competitor data.
- **No parser round-trip or crash-resistance tests on malformed source code.**
  The 44 `malformed`/`corrupt`/`truncated` tests all target *configuration and
  cache* parsing: `go.mod`, `package.json`, `package-lock.json`, `go.work`,
  baseline YAML, layer-cache blobs, the SQLite store. Nothing feeds a mutated
  `.ts` or `.go` file to the frontend.
- Partial mitigation: rule execution is wrapped in `catch_unwind`
  (`crates/polint/src/core/mod.rs:7694,7722`, `analysis_plan.rs:421,437`), so a
  panicking *rule* is contained. A panicking *frontend* is not.

**Danger:** for a tool whose entire input is untrusted third-party source code,
this is the single largest robustness gap. A malformed TSX file that panics the
parser is a denial of service on every consumer's CI.

### b.8 Soundness (false-negative) regression detection is real but micro-scale

There *is* a mechanism: `mode = "exact"` expected rows mean "this fact/edge MUST
be present," and `assert_eq!(false_negatives, 0)` fails if it disappears. That
genuinely distinguishes a missed edge from a crash. **But it operates over 381
assertions on 1,936 lines of toy source.** For the external suites — the only
place a real missed edge would show up — nothing is asserted at all (§b.1).

There is no soundness *proof* obligation anywhere: no "every call site must
resolve or be explicitly marked unknown" invariant checked at scale, no
comparison of `unknown_count` trends against a baseline in CI.

### b.9 No accuracy measurement on real repositories

`research/evaluation-harness/suites/` pins `grafana/grafana`,
`gohugoio/hugo`, `excalidraw/excalidraw`, and a private `devloupe` monorepo —
but only as **scale/latency** targets. There is no accuracy oracle for any real
repository, in any language. Every accuracy number polint has ever produced comes
from micro-benchmarks (Jelly's largest case, `helloworld`, is an Express app with
~20 npm deps; the rest are hand-written micro cases).

### b.10 Reproducibility by a third party: currently impossible

To reproduce 88.75% F1 an outsider must:
1. Discover the requirement (documented only in
   `performance/2026-06-17-array-model-refactor-plan.md:144-160`, a plan doc);
2. Clone `cs-au-dk/jelly` @ `b799ed4f` and `golang/tools` @ `7743a285` into
   gitignored paths;
3. Run **`npm install` in `repos/jelly/tests/helloworld/`** — unpinned by any
   lockfile in this repo. That case is 342 of 1,479 edges (23%), so **the largest
   case is not byte-reproducible across time**;
4. Set `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release` and run a
   named `#[cfg(test)]` function;
5. Read the number out of an uncommitted `.context/graph-benchmarks/` artifact.

If any step is skipped, the test passes silently.

Minor but telling: `jelly-callgraph-micro.toml:10` declares
`license = "BSD-3-Clause"` while the in-code manifest at
`jelly_callgraph.rs:2000` declares `Apache-2.0`. One is wrong.

---

## (c) Test-architecture assessment

### c.1 Inventory

| Metric | Value |
|---|---:|
| Total Rust LOC (`crates/`) | 267,710 |
| Total `#[test]` functions | **2,616** |
| — in `src/` (unit, in-module `#[cfg(test)]`) | **2,429** (93%) |
| — in `crates/polint/tests/` (integration) | **174** (7%) |
| `#[rstest]` | 0 |
| `#[cfg(test)]` occurrences | 777 |
| `#[ignore]` in `src/` | 2 |
| `#[test]` inside `src/eval/` | **361** (14% of all tests) |

Integration test files:

| File | LOC | Tests |
|---|---:|---:|
| `crates/polint/tests/cli.rs` | 12,166 | 166 |
| `crates/polint/tests/public_surface_leak.rs` | 632 | 7 |
| `crates/polint/tests/common/mod.rs` | 333 | 0 (helpers) |
| `crates/polint/tests/cargo_install_smoke.rs` | 75 | 1 (`#[ignore]`) |

**Assessment.** A 14:1 unit-to-integration ratio is heavily inverted for a tool
whose contract *is* its CLI output. `cli.rs` at 12,166 LOC / 166 tests (73
LOC/test) is dense but coherent — these are real temp-repo end-to-end tests
driving the binary via `assert_cmd`. The file is at the upper limit of
maintainability; splitting by subcommand would be reasonable but is not urgent.

The bigger structural issue: **361 tests (14%) test the test harness.** `eval/`
is 11% of the codebase and its files are dominated by `#[cfg(test)]` code
(`observed.rs`'s 4,027 lines compile to nothing in release). This is a large
maintenance surface producing little enforcement.

### c.2 Snapshot tests (insta): 5, all inline, effectively not snapshot testing

`insta` is a declared dependency (`Cargo.toml:45`) but there are **zero `.snap`
files in the repository**. All 5 usages are inline snapshots in
`crates/polint/src/diagnostics/mod.rs` (lines 2652, 2695, 2731, 2870, 3244),
using the `@r###"..."###` form.

Because there are no external snapshot files, there is no
`cargo insta review` workflow, no accept/reject step, and no risk of
rubber-stamping — but equally, there is **no snapshot coverage of analysis
output**. Diagnostic *rendering* is snapshotted; the CFG, call graph, data flow,
module graph, and evidence paths are not. Given the engine emits large
structured artifacts with a deterministic normalizer already implemented
(`eval/report.rs::to_deterministic_json_pretty`), this is a missed opportunity —
snapshotting normalized kernel output for the 53 fixtures would immediately
create thousands of reviewed regression assertions at near-zero cost.

### c.3 Property tests (proptest): 4, all infrastructure

| Test | Location | What it checks |
|---|---|---|
| `cache_key_for_file_path_participates_in_stable_id_proptest` | `crates/polint/src/cache/mod.rs:1195` | Distinct paths ⇒ distinct cache keys |
| `span_from_byte_range_is_monotonic_for_char_boundaries` | `crates/polint/src/core/mod.rs:11127` | Span construction is monotonic over char boundaries |
| `discovery_include_exclude_decision_is_stable` | `crates/polint/src/fs/mod.rs:302` | Glob include/exclude decisions are stable |
| `sort_diagnostics_is_input_order_independent` | `crates/polint/src/diagnostics/mod.rs:3622` | Diagnostic sort is order-independent |

All four are plumbing invariants. **None exercise analysis semantics.** The
obvious high-value property tests are absent:
- parser never panics on arbitrary UTF-8 / arbitrary token soup;
- CFG is always a connected graph with exactly one entry and reachability
  closed under edges;
- call graph node/edge IDs are stable under file reordering and whitespace;
- data-flow results are monotone under adding an unrelated statement;
- incremental (warm) run ≡ cold run for the same input (this is *partially*
  covered by `check_cached_output_is_deterministic` in `cli.rs:10828` but as a
  fixed-input test, not a property).

### c.4 Test discipline is otherwise strong

- `deny_unknown_fields` on every eval schema struct — a typo in a fixture TOML
  is a hard error, not a silently-ignored field.
- Fixture manifests are schema-versioned (`polint-eval-fixture-1`) and validated
  on load (`fixtures.rs:60-83`).
- Determinism is treated as a first-class property with a dedicated gate (§d.2).
- CI runs the full suite on Linux, macOS, and Windows — genuine platform parity,
  not "Linux plus hope".

---

## (d) Mechanically-enforced architectural invariants — the real strength

This is where polint is genuinely above average. These are not documentation;
they are tests that fail a PR.

### d.1 Public-surface leak gate — `crates/polint/tests/public_surface_leak.rs` (632 LOC, 7 tests)

The strongest single artifact in the repo. It runs on **Linux and macOS
independently** (`ci.yml:182-183`), and the header comment
(`public_surface_leak.rs:13-14`) explicitly forbids averaging or skipping either.

| Test | Invariant |
|---|---|
| `probe_crate_compiles_against_prelude_only` (`:337`) | An excluded probe crate at `tests/fixtures/public-surface-leak-probe/` with `#![no_implicit_prelude]` and a single `use ::polint::sdk::prelude::*;` compiles cleanly. Shells out to `cargo build` for rustc-level granularity — deliberately without adding `trybuild` as a dependency (`:16-21`). |
| `allowlist_matches_prelude_source` (`:382`) | The parsed `pub mod prelude { … }` block of `sdk/mod.rs` equals `ALLOWED_PRELUDE` **exactly** (109 entries, `:41-164`). Any addition trips the gate and requires a documented promotion record in `docs/API-VISIBILITY-PLAN.md`. |
| `ensure_no_private_namespace_in_probe` (`:417`) | The probe never reaches into `polint::analysis*`, `polint::eval`, `polint::core`, … (`:469`) |
| `semantic_store_markers_do_not_leak_into_supported_public_surfaces` (`:487`) | 13 forbidden markers (`SemanticStore`, `rusqlite`, `PRAGMA user_version`, `SQLITE_OPEN_READ_WRITE`, `sqlite_row_id`, …, `:191-205`) appear in no `.rs`/`.md` under the public tree |
| `semantic_store_marker_scanner_negative_controls_cover_every_family` (`:544`) | The scanner itself is proven to detect each marker family — no silently-broken scanner |
| `parser_self_test_detects_synthetic_leak` (`:568`) | **The prelude parser is tested against a synthetic leak** so a buggy parser cannot hollow out the gate. This is the meta-invariant most projects forget. |
| `allowlist_has_no_duplicates_and_expected_count` (`:616`) | Allowlist integrity |

The pattern — *gate + negative control for the gate + self-test of the gate's
parser* — is exactly right and should be the template for the accuracy gates
that don't yet exist.

### d.2 Determinism gate — `crates/polint/src/eval/determinism_gate.rs` (541 LOC)

`N = 10` seeded permutations of (a) provider execution order and (b) provider
output-row insertion order must produce **byte-identical normalized JSON**
(`determinism_gate.rs:1-18`, `PERMUTATION_RUNS = 10` at `:60`). Uses an in-tree
SplitMix64 PRNG to avoid adding a `rand` dependency (`:63-80`).

The best property: **near-zero-maintenance auto-enrollment**
(`determinism_gate.rs:21-27`). The shuffled provider set is sourced directly
from `AnalysisKernel::provider_manifests()`, and the shuffled count is asserted
equal to `provider_manifests().len()`. A newly registered provider is
automatically covered; drifting out of sync is impossible. This is a genuinely
excellent design.

Runs on ubuntu + macos (`ci.yml:185-186`).

### d.3 Other mechanically-enforced invariants

| Invariant | Enforced by |
|---|---|
| Polyglot Go+TS non-interference — the shared solver core propagates within a language but no derived edge crosses the Go↔TS boundary | `crates/polint/src/eval/go_rta.rs:20-26`; `ci.yml:179-180` |
| Semantic-store memory/latency budget: ≤ +20% peak RSS, ≤ +25% cold wall clock vs the committed store-disabled baseline; plus a `diagnostics_digest` equality check | `crates/polint/src/eval/baseline.rs:199,205`; `bench/gate.rs`; `ci.yml:188-192` (serialized, `--test-threads=1`) |
| Byte-identical `check` output with the store enabled / disabled / corrupted | store gate `diagnostics_digest` comparison |
| No project-management references (phase numbers, plan IDs) in shipped source comments | `AGENTS.md:100`; enforced by tests in `cli.rs` |
| `polint eval` never appears in README/SDK/docs/generated skill text | `crates/polint/src/eval/runner.rs:1004` |
| Framework-entrypoint, refined-call, CFG, data-flow, and type-value-alias internals do not leak into public surfaces | `analysis_kernel/mod.rs:2476,2522,2567`; `cli.rs:1755,1809,1883,3254,3260` |
| SARIF output is valid 2.1.0 with correct rule-property tagging | `ci.yml:222-233` (Python validator) + upload to GitHub Code Scanning |
| `cargo install --locked` from a clean tree produces a working binary | `crates/polint/tests/cargo_install_smoke.rs`; `ci.yml:196-204` |
| MSRV 1.95 holds across the workspace with all features | `ci.yml:58-68` |
| Zero clippy warnings across all targets and features; zero rustdoc warnings | `ci.yml:70-87`, `ci.yml:33-43` |
| No advisories / license violations / banned deps | `cargo-deny`, `ci.yml:45-54` |
| A panicking rule cannot take down a run | `catch_unwind` at `core/mod.rs:7694,7722`, `analysis_plan.rs:421,437` |
| Fixture manifests reject unknown fields and unversioned schemas | `serde(deny_unknown_fields)` throughout `eval/model.rs`; `fixtures.rs:60-83` |
| `synthetic_observed` is confined to 3 whitelisted areas and forbidden in evidence fixtures | `fixtures.rs:109-125`, `fixtures.rs:1616-1618` |
| Deterministic rule-template scaffolding | `cli.rs:7066` |
| Deterministic cached / parallel-cached / JSON output | `cli.rs:10828,10859,11067` |

That is a strong list. The discipline that produced it is exactly what an
accuracy gate needs — it just hasn't been pointed at accuracy.

---

## (e) What was recommended vs what was built

`research/evaluation-harness/FINAL-REPORT.md` (2026-05-26) recommended five
suites and a public-claim policy.

| Recommendation | Status |
|---|---|
| Native polint fixtures as first promotion gate (`FINAL-REPORT.md:19-21`) | **Built**, but at 1,936 LOC of toy source with recall-only assertions |
| Go x/tools RTA as primary external Go benchmark (`:22-23`) | **Built**; last measured 2026-06-06; not in CI |
| Jelly as primary external JS/TS benchmark (`:24-25`) | **Built and heavily invested in**; F1 88.75%; not in CI; not reproducible |
| SecBench.js as external TS/JS scanner benchmark (`:26`) | **Stub only** — constant `unlabelled-executable-case` label |
| gosec as external Go scanner benchmark (`:27-28`) | **Stub only** — constant `sample-native-label` label |
| Competitor rows (Jelly, x/tools, Semgrep, CodeQL, gosec) (`:29-30`) | **Schema only** (`competitors.rs`), zero data |
| Adaptation rows with prompt hash, budget, accepted/rejected facts (`:31-33`) | **Schema built** (`adaptation.rs`, 416 LOC); no runs |
| Report TP/FP/FN/TN, P/R/F1/F2/F3, FPR, unknowns, runtime, memory (`:47-54`) | **Computed** in `metrics.rs`; **never asserted** |
| *"Public precision claims must cite measured reports from supported suites"* (`:56-60`) | **Honored** — the README makes no accuracy claims at all |

`research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` set target
score bands (`:491-505`). Measured against them:

| Milestone band | Jelly precision target | Jelly recall target | Actual (2026-06-17) |
|---|---:|---:|---|
| Current baseline (2026-05) | 25% | 1% | — |
| Go RTA / JS token solver | 50-80% | 35-60% | — |
| JS object/property model + adaptation | 55-85% | 55-80% | **P 96.14%, R 82.42%** |

**polint has met or beaten the top planned band on the Jelly suite.** That is a
real achievement — and it is invisible to CI, absent from the README, absent
from the committed baseline, and unverifiable by anyone outside this machine.
That gap between the engineering reality and the evidentiary record is the core
finding of this review.

---

## (f) Target evaluation architecture

### f.1 Benchmark suites: one oracle per analysis per language

| Analysis | Language | Suite | Type |
|---|---|---|---|
| Call graph | JS/TS | Jelly (76 cases / 1,479 edges) — **vendored, pinned, lockfiled** | Have; needs custody |
| Call graph | Go | Go x/tools RTA (5 cases / 37 edges) + generated RTA oracle over real repos via `golang.org/x/tools/go/callgraph/rta` | Have (micro); need scale |
| Call graph | JS/TS + Go | **Real-repo tier**: `excalidraw`, `hugo`, `grafana` with a *sampled, human-adjudicated* edge oracle (500-1000 edges each) | **Missing** |
| Points-to / alias | JS/TS, Go | PointerBench-style micro suite, ported (~50 cases with exact expected points-to sets) | **Missing** |
| Taint / data flow | JS/TS | OWASP Benchmark-equivalent for JS (or port the Juliet C/Java patterns), plus SecuriBench-Micro semantics: ~200 cases, each with a *labelled* TP or FP verdict | **Missing** — this is the biggest gap |
| Taint / data flow | Go | gosec's own testdata with **real per-rule G-code labels** (they exist upstream; the adapter just doesn't read them) | Stub → real |
| Security rules end-to-end | JS/TS, Go | SecBench.js with real CVE labels (the repo has 704 `.test.js` cases; they need mapping) | Stub → real |
| CFG | JS/TS, Go | Structural property suite (single entry, reachability closure, dominator sanity) over the *real-repo* corpus, not fixtures | **Missing** |
| Module graph / imports | JS/TS, Go | Differential vs `tsc --traceResolution` and `go list -deps` — free ground truth, no labelling required | **Missing, and cheap** |
| Incremental correctness | all | Warm ≡ cold equivalence over N random edit sequences on real repos | Partial |

**Priority note.** polint ships 10 security templates and positions itself on
policy enforcement. It has *zero* taint ground truth. A SecuriBench/OWASP-class
suite is not a nice-to-have; it is the difference between a marketing claim and
a measurement.

### f.2 CI gates

Three tiers, all fail-loud:

1. **Every PR (~2 min added).** Micro-oracle tier: Jelly `fast` (20 cases) +
   Go x/tools (5 cases) + the taint micro suite, run against a **vendored,
   git-LFS or submodule-pinned corpus** so it can never silently skip. Gate:
   `F1 >= committed_baseline - 0.5pp` AND `precision >= committed_baseline - 0.5pp`
   AND `false_positives <= committed_baseline * 1.05`. Wire this through the
   *existing* `evaluate_promotion_gates` — the code is already written and
   tested; it just needs a production caller and a real `SuiteGateConfig`.
2. **Nightly (~30 min).** Full Jelly `release` (76 cases), full Go RTA, full
   taint suite, real-repo call-graph sample. Commit the resulting
   `EvaluationRun` JSON to a `benchmarks/` history directory so the trend is a
   reviewable diff, not a lost artifact.
3. **Release.** Everything, plus a signed, dated scorecard published to the docs
   site with corpus commit hashes, polint version, and the exact command.

**Delete the silent `return`.** Replace every
`if !repo.exists() { return; }` with a hard failure when
`POLINT_REQUIRE_BENCH_CORPUS=1` (set in CI), and a `println!("SKIP: …")` plus
`#[ignore]` semantics otherwise. A benchmark that can silently not run is worse
than no benchmark, because it manufactures false confidence.

### f.3 Public leaderboard

A `BENCHMARKS.md` at repo root, regenerated by CI, containing:
- one row per (suite, tier, polint version, corpus commit) with TP/FP/FN,
  P/R/F1, runtime, peak RSS;
- the exact reproduction command, including corpus provenance;
- **both pruned and unpruned Jelly numbers** — publishing only the reachability-
  pruned 88.75% while the raw agreement is 66.2% will not survive contact with a
  skeptical reader. Publish both and explain the normalization; that is more
  credible than either number alone.
- competitor rows populated from `competitors.rs` — the schema already demands
  product version, suite commit, and a non-empty `limitations` field
  (`competitors.rs:22-63`). Actually filling it with Jelly, `go/callgraph`,
  Semgrep, and CodeQL runs is the single highest-credibility artifact available.

### f.4 Differential + fuzz testing

| Technique | Target | Effort |
|---|---|---|
| **Fuzz the frontends** (`cargo-fuzz` + `arbitrary`): TS/JS parser, Go parser, `go.mod`/`package.json`/`package-lock.json` readers, SARIF/JSON emitters | No panic, no OOM, no infinite loop on arbitrary bytes | 1-2 days for first targets; highest robustness ROI |
| **Structure-aware fuzzing**: mutate the 1,936 LOC of fixtures + real-repo files (delete/duplicate/reorder tokens) and assert no panic and monotone unknown counts | Frontend robustness | 2-3 days |
| **Differential: module resolution** vs `tsc --traceResolution` / `go list -deps -json` | Import resolution correctness at zero labelling cost | 2-3 days, very high value |
| **Differential: call graph** vs Jelly binary and `x/tools` RTA, run live rather than against checked-in JSON | Catches oracle-format drift and normalization bugs | 3-5 days |
| **Metamorphic testing**: renaming a symbol must not change edge count; adding a dead function must not change existing edges; reordering top-level declarations must not change output | Deep semantic invariants, no ground truth needed | 2-3 days, exceptional ROI |
| **Warm ≡ cold as a property test** over random edit sequences | Incremental soundness | 2-3 days |

Metamorphic testing deserves emphasis: it converts the existing 53 fixtures and
the real-repo corpus into thousands of assertions with **no labelling cost**,
and it catches exactly the class of bug (identity instability, order sensitivity)
that the determinism gate proves the team already cares about.

### f.5 Soundness accounting

Add a standing "unknown budget" report: for each suite, the count of call sites
that resolved / resolved-partially / were explicitly marked unknown / were
silently dropped. **The last bucket should be provably zero.** Gate on it. This
is the only way to distinguish "we don't know" (honest, already a first-class
concept in this codebase per the Honesty Gate at
`.planning/REQUIREMENTS.md:30`) from "we forgot".

---

## (g) Prioritized first steps

Ordered by (credibility gained) ÷ (effort). The first three are days, not weeks,
and they convert existing work into enforceable evidence.

**P0 — Stop the silent skip and freeze the number. (~1 day)**
1. Replace the bare `return` at `crates/polint/src/eval/external/mod.rs:27-29`
   (and the four sibling sites) with an explicit skip message, and make skipping
   a hard failure under `POLINT_REQUIRE_BENCH_CORPUS=1`.
2. Regenerate `research/evaluation-harness/baselines/persisted-graph-accuracy.json`
   with the real 1219/49/260 numbers. Un-check BENCH-04 until this lands.
3. Add `assert!(jelly.metrics.f1 >= baseline.f1 - 0.005)` to the external test.
   The metric is already computed at `metrics.rs:245-268`; this is a three-line
   change that turns 29,344 LOC of harness into a gate.

**P1 — Make the corpus custodial. (~2 days)**
4. Vendor the Jelly oracle JSONs (they are small — 76 files) into
   `tests/eval-fixtures/jelly-oracle/` under their BSD-3-Clause license, so the
   1,479-edge ground truth is in-tree and version-controlled. Resolve the
   BSD-3-Clause vs Apache-2.0 metadata contradiction
   (`jelly-callgraph-micro.toml:10` vs `jelly_callgraph.rs:2000`).
5. Commit the `helloworld` `package-lock.json` so the 342-edge case is
   byte-reproducible.
6. Add a `make bench-corpus` target that clones and pins everything, and document
   it in the README rather than in a plan doc.

**P2 — Wire the gate that already exists. (~2 days)**
7. Give `evaluate_promotion_gates` a production caller. Add
   `[gates]` sections with `precision_floors` and
   `required_per_language_deltas` to the suite manifests. The 746 lines of
   `gates.rs` are already written and unit-tested against 9 scenarios; they need
   configuration and a call site, not new code.
8. Add the Jelly `fast` tier (20 cases) to the `gates` CI job. Budget: ~2 min.

**P3 — Close the taint hole. (~1-2 weeks)**
9. Build a SecuriBench-Micro-equivalent for JS/TS: ~150-200 cases across the 10
   shipped template categories, each labelled TP or FP, each with a *distinct*
   sanitizer name from the rule's barrier list so the fixture cannot pass by
   name-matching. Include explicit FP traps (sanitized flows, unreachable sinks,
   type-narrowed paths) using the already-implemented but unused
   `AssertionMode::Forbidden` and `false_positive_trap`.
10. Read the real per-rule G-codes out of gosec's testdata instead of emitting
    `"gosec/sample-native-label"` (`gosec.rs:132-139`). The labels exist upstream.

**P4 — Robustness. (~1 week)**
11. Add `cargo-fuzz` with three targets: TS/JS parse, Go parse, manifest parse.
    Wire a 5-minute nightly run. Seed from the existing fixture corpus.
12. Add metamorphic property tests (rename-invariance, dead-code-invariance,
    declaration-reorder-invariance) over the 53 fixtures.

**P5 — Publish. (~3 days)**
13. Generate `BENCHMARKS.md` in CI with pruned and unpruned numbers, corpus
    commits, and reproduction commands.
14. Populate `competitors.rs` with at least one real head-to-head row per suite.

---

## Summary judgement

**Strengths.** The mechanically-enforced invariant suite (§d) is better than most
commercial static analysis tools — particularly the public-surface leak gate with
its parser self-test, and the determinism gate with automatic provider
enrollment. The engineering that took Jelly F1 from 1.07% to 88.75% in eleven
days is real and, per the project's own planning targets, above the top forecast
band. The repo is unusually honest with itself: `baselines/README.md:51-54`
explicitly warns against claiming unmeasured recall lifts, and the README makes
no accuracy claims at all.

**Weaknesses.** The evaluation program is a large, well-designed apparatus that
is not connected to a circuit breaker. Nothing asserts accuracy. The benchmark
silently no-ops in CI. The committed baseline is null. The promotion gates have
no production caller. The taint engine — which backs ten shipped security
templates — is validated by one 32-line fixture and twenty self-generated
snippets. There is no fuzzing and no differential testing.

**The one-sentence version:** polint is measuring far less than it has built the
capacity to measure, and the gap between its real engineering progress and its
verifiable evidentiary record is now its largest strategic risk.
