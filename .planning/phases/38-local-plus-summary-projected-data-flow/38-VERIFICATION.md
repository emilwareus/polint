---
phase: 38-local-plus-summary-projected-data-flow
verified: 2026-05-25T11:29:42Z
status: gaps_found
score: 5/10 must-haves verified
overrides_applied: 0
gaps:
  - truth: "Phase 38 provides local value-flow edges, not only local place nodes."
    status: failed
    reason: "The provider mirrors MIR places into DataFlowNodeFact rows but does not derive assignment/use/field/access-path local DataFlowEdgeFact rows."
    artifacts:
      - path: "crates/polint/src/analysis/data_flow/provider.rs"
        issue: "derive_local_place_nodes creates place nodes; no local MIR edge derivation exists."
      - path: "crates/polint/src/analysis/data_flow/"
        issue: "No local.rs or equivalent local-flow builder exists."
    missing:
      - "Derive local value-flow edges from MIR operations and places for assignments, uses, returns, and bounded field/property/access-path projections."
      - "Emit explicit unknown or budget rows for unsupported local operations instead of silently omitting flow."
      - "Add focused local-flow tests over real Go and TS/JS lowered MIR."
  - truth: "Phase 38 provides summary-projected data-flow edges and missing-summary unknown/havoc rows."
    status: failed
    reason: "The vocabulary has SummaryProjection/SummaryTito variants, but the provider does not consume direct summary rows to create summary-projected edges or unknown/havoc rows."
    artifacts:
      - path: "crates/polint/src/analysis/data_flow/provider.rs"
        issue: "derive_data_flow_with_cache_stats receives direct_summaries_output_digest for identity only; it does not read summaries from AnalysisDb."
    missing:
      - "Project direct summary TITO/effect rows into compact data-flow edges."
      - "Represent missing, unsupported, setup-missing, or budget-exceeded summaries as explicit data-flow status rows."
      - "Add regression tests that fail if summary rows are only included in cache identity but not emitted as facts."
  - truth: "Phase 38 provides budget/unknown/havoc facts visible through stored data-flow output."
    status: partial
    reason: "Query search returns BudgetExceeded statuses in memory, and DataFlowBudgetFact exists, but the provider does not store budget facts or unknown/havoc facts for analysis output."
    artifacts:
      - path: "crates/polint/src/analysis/data_flow/query.rs"
        issue: "Path budget status is private query output only."
      - path: "crates/polint/src/analysis/data_flow/provider.rs"
        issue: "Provider does not populate DataFlowBudgetFact rows or Unknown/SetupMissing/BudgetExceeded data-flow facts."
    missing:
      - "Store deterministic DataFlowBudgetFact rows for provider/path budgets when limits affect results."
      - "Store unknown/havoc facts for unsupported local, call, summary, and model cases."
  - truth: "Phase 38 eval fixtures cover data-flow behavior, determinism, and taxonomy."
    status: failed
    reason: "There is no tests/eval-fixtures/data-flow directory, and the plan-listed eval test filters run zero tests."
    artifacts:
      - path: "tests/eval-fixtures/"
        issue: "No data-flow fixture exists."
      - path: ".planning/phases/38-local-plus-summary-projected-data-flow/38-07-PLAN.md"
        issue: "Lists eval_native_fixture_runner_data_flow_fixture_passes and eval_data_flow_manifests_cover_required_taxonomy, but those tests are not present."
    missing:
      - "Add native data-flow eval fixtures for local flow, direct-call projection, summary projection, model facts, extension/rejection behavior, budgets, unknowns, and deterministic output."
      - "Add real eval test names and make the plan filters run non-zero tests."
  - truth: "Phase 38 has a dedicated public no-leak proof for data-flow internals."
    status: partial
    reason: "The data-flow provider remains crate-private and the SDK DataFlow view is still reserved/unsupported, but no dedicated data_flow_public_no_leak test exists and docs/facts/data-flow.md intentionally mentions polint.data_flow."
    artifacts:
      - path: "crates/polint/src/sdk/facts.rs"
        issue: "DataFlow remains a reserved unsupported view, which is correct."
      - path: "docs/facts/data-flow.md"
        issue: "Public docs mention the private provider id and internal fact names without a corresponding no-leak policy test."
      - path: "crates/polint/src/analysis_kernel/mod.rs"
        issue: "Existing no-leak tests cover earlier families but not Phase 38 data-flow markers."
    missing:
      - "Add data-flow-specific public boundary tests for check JSON, CLI help, SDK exports, runner surface, README, and docs/facts."
      - "Decide whether docs/facts/data-flow.md is an intentional internal-limit document or should be deferred until Phase 41 promotion."
---

# Phase 38: Local Plus Summary-Projected Data Flow Verification Report

**Phase Goal:** Add local/interprocedural value flow, model sinks, unknowns, budgets, and query-scoped path search.
**Requirement:** SAE-PREC-03
**Verified:** 2026-05-25T11:29:42Z
**Status:** gaps_found
**Re-verification:** No - initial critical verification after all Phase 38 summaries were present.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | A private `analysis::data_flow` module exists and is not a public SDK/CLI provider surface. | VERIFIED | `crates/polint/src/analysis/mod.rs` declares `pub(crate) mod data_flow`; SDK `DataFlow<'_>` remains reserved/unsupported in `sdk/facts.rs`. |
| 2 | Data-flow fact vocabulary exists for nodes, edges, models, budgets, precision, status, validation, confidence, and provenance. | VERIFIED | `facts.rs` defines `DataFlowNodeFact`, `DataFlowEdgeFact`, `DataFlowModelFact`, `DataFlowBudgetFact`, status/precision/validation/provenance enums, and normalization helpers. |
| 3 | Stored output normalizes stable-key order, reassigns dense IDs, validates dangling references, and refreshes metadata. | VERIFIED | `store.rs` normalizes nodes/models/budgets/edges; focused `data_flow` tests passed. `core/mod.rs` stores data-flow rows and metadata. |
| 4 | `polint.data_flow` runs after refined calls and before metrics with provider output identity. | VERIFIED | Kernel provider order includes `polint.data_flow` at slot 15 before metrics; `cargo test -p polint --lib provider_order --locked` passed. |
| 5 | Cache identity includes provider/schema, upstream digests, deterministic parameters, and input snapshot sentinels. | VERIFIED | `cache_key.rs` and `provider.rs` compute parameter/input/output digests, including upstream provider output digests and absent model/extension/tool sentinels. |
| 6 | Local data-flow has real local value-flow edges over MIR operations. | FAILED | The provider creates `DataFlowNodeKind::Place` rows from `AnalysisDb::mir_places()`, but no local assignment/use/field/access-path edge builder exists. |
| 7 | Direct/refined call projection creates interprocedural data-flow facts. | PARTIAL | Resolved refined call edges create call argument/return nodes and `CallArgumentToReturn` edges. Argument-to-parameter, receiver, return-use, and summary event bindings are not implemented. |
| 8 | Summary-projected flow and missing-summary unknown/havoc behavior are present. | FAILED | Direct summary output digest participates in identity, but summary rows are not consumed to emit data-flow edges or unknown/havoc rows. |
| 9 | Source/sink/sanitizer/barrier models are represented with provenance. | PARTIAL | Trust boundaries become source models/nodes and extension facts may become source/sink/sanitizer/barrier/TITO models. Native sinks/sanitizers/barriers and validation-gated rejected model rows are not proven. |
| 10 | Budgets, unknowns, eval fixtures, debug/no-leak proof, and bounded path search satisfy the Phase 38 requirement. | PARTIAL | Bounded path search has unit tests and budget status in memory. Stored budget/unknown/havoc facts, data-flow eval fixtures, data-flow debug snapshots, and dedicated public no-leak tests are missing. |

**Score:** 5/10 must-haves verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/polint/src/analysis/data_flow/facts.rs` | Fact vocabulary | VERIFIED | Defines the core row and enum vocabulary. |
| `crates/polint/src/analysis/data_flow/store.rs` | Deterministic storage and indexes | VERIFIED | Normalization/reference validation exists and focused tests pass. |
| `crates/polint/src/analysis/data_flow/cache_key.rs` | Provider parameter/input identity | VERIFIED | Deterministic settings and snapshot digest helpers exist. |
| `crates/polint/src/analysis/data_flow/provider.rs` | Private provider | PARTIAL | Wires provider, local place nodes, refined-call projection, trust-boundary sources, and extension models; does not implement local edge derivation, summary projection, budget rows, or unknown/havoc rows. |
| `crates/polint/src/analysis/data_flow/query.rs` | Query-scoped path search | PARTIAL | Bounded BFS exists and has focused tests, but it is not integrated with stored budget/query observation rows. |
| `crates/polint/src/analysis/data_flow/validate.rs` | Validation hook | PARTIAL | Duplicate stable keys and store validation are available, but broader Phase 38 validation criteria are not covered. |
| `tests/eval-fixtures/data-flow/` | Native data-flow eval fixtures | MISSING | Directory does not exist. |
| `crates/polint/src/eval/fixtures.rs` | Data-flow fixture runners/taxonomy | MISSING | No `eval_native_fixture_runner_data_flow_fixture_passes` or `eval_data_flow_manifests_cover_required_taxonomy` test exists. |
| Public no-leak tests | Dedicated data-flow no-leak proof | MISSING | Existing no-leak tests cover older families, not data-flow markers. |
| `docs/facts/data-flow.md` | Honest documentation of current status | PARTIAL | States private/heuristic limits, but this conflicts with the plan's requirement that docs/facts not leak private provider IDs unless intentionally promoted. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Formatting | `cargo fmt --all --check` | Passed | PASS |
| Focused data-flow tests | `cargo test -p polint --lib data_flow --locked` | 11 passed, 0 failed | PASS |
| Provider-order proof | `cargo test -p polint --lib provider_order --locked` | 7 passed, 0 failed | PASS |
| Plan-listed data-flow eval fixture filter | `cargo test -p polint --lib eval_native_fixture_runner_data_flow_fixture_passes --locked` | 0 tests run | FAIL |
| Plan-listed data-flow taxonomy filter | `cargo test -p polint --lib eval_data_flow_manifests_cover_required_taxonomy --locked` | 0 tests run | FAIL |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-PREC-03 | 38-01 through 38-07 | polint has local and summary-projected data-flow facts, source/sink/sanitizer/barrier model sinks, budgets, unknown/havoc facts, and query-scoped path search. | PARTIAL | The private substrate, provider order, cache identity, source models, extension model ingestion, and bounded query search exist. Local value-flow edges, summary projection, stored unknown/budget/havoc facts, data-flow eval fixtures, and dedicated no-leak proof are missing. |

### Critical Review Findings

| Finding | Severity | Evidence | Required Closure |
|---|---|---|---|
| Local flow is node-only, not value-flow. | Blocking | `derive_local_place_nodes` mirrors places; no local edge builder exists. | Add local MIR edge derivation and unsupported/unknown rows. |
| Summary projection is identity-only. | Blocking | Provider receives `direct_summaries_output_digest` but does not read summary rows. | Consume summary facts and emit summary-projected or unknown/havoc rows. |
| Stored budget/unknown/havoc evidence is missing. | Blocking | `DataFlowBudgetFact` exists but provider never populates it; query budget status is in-memory only. | Store deterministic budget and uncertainty facts where budgets or unsupported semantics affect flow. |
| Eval coverage claimed by the plan is absent. | Blocking | Data-flow eval test filters run zero tests; no data-flow fixture directory exists. | Add fixture(s), taxonomy tests, and non-zero focused filters. |
| Public-boundary proof is incomplete. | Major | No data-flow-specific no-leak test; docs/facts mentions `polint.data_flow`. | Add explicit boundary tests and settle docs exposure policy. |

### Human Verification Required

None. This phase is internal Rust analysis behavior and can be verified through source inspection and automated tests.

### Gaps Summary

Phase 38 is not finished against SAE-PREC-03. It establishes a useful private data-flow substrate and a provider skeleton, but it does not yet deliver the local plus summary-projected data-flow behavior the roadmap requires.

Recommended next action: create and execute a Phase 38 gap-closure plan focused on real local edges, summary projection, stored budget/unknown rows, data-flow eval fixtures, and public-boundary proof. Do not advance to Phase 39 until this report passes.

---

_Verified: 2026-05-25T11:29:42Z_
_Verifier: Codex_
