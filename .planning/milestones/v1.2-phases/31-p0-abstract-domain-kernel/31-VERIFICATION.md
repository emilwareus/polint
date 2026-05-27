---
phase: 31-p0-abstract-domain-kernel
verified: 2026-05-21T13:26:09Z
status: passed
score: 12/12 must-haves verified
overrides_applied: 0
---

# Phase 31: P0 Abstract-Domain Kernel Verification Report

**Phase Goal:** Add deterministic abstract-domain infrastructure and first local domains over MIR/CFG.
**Requirement:** SAE-INT-01
**Verified:** 2026-05-21T13:26:09Z
**Status:** passed
**Re-verification:** No - initial phase verification after code-review fixes

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | Abstract-domain internals are crate-private and not visible to rule authors. | VERIFIED | `analysis::domains` is registered as `pub(crate)` and public-boundary tests assert no abstract-domain SDK, runner, CLI, README, docs/facts, check JSON, inspect JSON, or `polint test` output leak. |
| 2 | P0 domain slots exist for reachability, nilness/nullishness, truthiness, constants, strings, and initializedness. | VERIFIED | `core.rs` implements all six domain slots with law-focused tests; `cargo test -p polint --lib analysis::domains --locked` and full regression passed. |
| 3 | Domain contracts include bottom/top, partial order, join, join_into, widening, and stable digest behavior. | VERIFIED | `lattice.rs` defines `AbstractDomain`, `TopReason`, widening hooks, and deterministic digest parts; domain law tests passed. |
| 4 | Product state is deterministic and uses stable ordering for place-indexed slots. | VERIFIED | `state.rs` uses sorted product state and stable digest parts; insertion-order and reduction tests passed. |
| 5 | Local solving is deterministic over MIR/CFG/calls regardless of input row order. | VERIFIED | `LocalDomainSolver` uses sorted solve units and deterministic worklists; shuffled-row and repeated-solve digest tests passed. |
| 6 | Transfer behavior is separate from lattice operations and conservative for calls, unsupported semantics, dynamic writes, and budgets. | VERIFIED | `transfer.rs` implements operation and edge transfer over polint-owned MIR/CFG/call facts only, with top/unknown/budget tests and monotonicity samples. |
| 7 | Domain results are cursor-queryable by function/block/operation/place without exposing mutable internals. | VERIFIED | `DomainResults` exposes entry, block entry/exit, before/after operation, stable iterators, place observations, and top events; results tests passed. |
| 8 | Domain facts are stored with stable keys, statuses, precision, metadata, and deterministic indexes. | VERIFIED | `facts.rs`, `store.rs`, and `core/mod.rs` add domain observation/event rows, `DomainStore` indexes, and `replace_abstract_domain_facts`; storage and metadata tests passed. |
| 9 | `polint.abstract_domains` runs after calls and before metrics with deterministic provider output identity. | VERIFIED | Kernel run-report, provider-order fixture, and schema evidence show `polint.abstract_domains` between `polint.calls` and `polint.metrics`; provider tests passed. |
| 10 | Cache identity includes domain versions, policy, upstream MIR/CFG/calls inputs, lifecycle/config, and absent future slots. | VERIFIED | `cache_key.rs` and `analysis_kernel/incremental/keys.rs` define abstract-domain provider parameters and layer keys; layer-key/provider-parameter tests passed. |
| 11 | Validation and debug output fail closed and avoid raw source, AST dumps, absolute paths, and public internals. | VERIFIED | `validate_abstract_domains` checks references, stable keys, status/reason consistency, and precision ceilings; debug JSON tests and review fixes verified generic public diagnostics. |
| 12 | Internal eval proves domain rows, uncertainty states, budget states, determinism, and public no-leak behavior. | VERIFIED | `tests/eval-fixtures/abstract-domains/core` covers domain observations/events and cold/warm/no-cache determinism; public no-leak integration test passed. |

**Score:** 12/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/polint/src/analysis/domains/lattice.rs` | Private lattice contracts | VERIFIED | Defines `AbstractDomain`, `Changed`, `TopReason`, widening types, and digest contracts. |
| `crates/polint/src/analysis/domains/core.rs` | P0 domain slots | VERIFIED | Implements reachability, nilness/nullishness, truthiness, constants, strings, and initializedness. |
| `crates/polint/src/analysis/domains/state.rs` | Deterministic product state | VERIFIED | Provides `CoreDomains`, `ProductState`, place-indexed slots, reductions, joins, widening, and digests. |
| `crates/polint/src/analysis/domains/solver.rs` | Deterministic local solver | VERIFIED | Provides sorted solve units, deterministic worklist, widening fuel, and budget states. |
| `crates/polint/src/analysis/domains/transfer.rs` | MIR/CFG/call transfer | VERIFIED | Handles literals, branches, calls, unsupported operations, dynamic writes, and top reasons. |
| `crates/polint/src/analysis/domains/results.rs` | Result cursor semantics | VERIFIED | Exposes stable-key ordered result access for entries, blocks, operations, places, and top events. |
| `crates/polint/src/analysis/domains/facts.rs` | Domain row vocabulary | VERIFIED | Defines observation/event facts, slots, locations, statuses, precision, and values. |
| `crates/polint/src/analysis/domains/store.rs` | Domain storage and indexes | VERIFIED | Normalizes rows, stores metadata, preserves uncertainty rows, and indexes by stable dimensions. |
| `crates/polint/src/analysis/domains/provider.rs` | Private provider | VERIFIED | Solves domains, converts results, stores facts, and records deterministic output digests. |
| `crates/polint/src/analysis/domains/cache_key.rs` | Provider parameter digest | VERIFIED | Covers schema, versions, reduction/widening policy, budgets, and future absent inputs. |
| `crates/polint/src/analysis/domains/validate.rs` | Domain validation | VERIFIED | Rejects malformed rows, dangling references, duplicate stable keys, and invalid status/precision combinations. |
| `crates/polint/src/analysis_kernel/debug.rs` | Test-facing debug JSON | VERIFIED | Emits compact `abstract_domains` observations, events, counts, and index counts. |
| `crates/polint/src/eval/observed.rs` and `crates/polint/src/eval/fixtures.rs` | Eval observation and fixture runners | VERIFIED | Normalize domain debug rows and run the abstract-domain fixture with determinism checks. |
| `tests/eval-fixtures/abstract-domains/core/expected.polint-eval.toml` | Native eval fixture expectations | VERIFIED | Covers observations/events, domain slots, top/unknown/budget rows, counts, indexes, and determinism. |
| `crates/polint/tests/cli.rs` | Public no-leak proof | VERIFIED | `abstract_domain_internals_stay_private` passed in focused and full regression runs. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `analysis::domains::solver` | MIR/CFG/calls rows | `SolverInput` and deterministic sorted solve units | WIRED | Solver consumes private rows and produces stable `DomainResults`. |
| `analysis::domains::transfer` | P0 domain state | MIR operation and CFG edge transfer | WIRED | Transfer updates product state conservatively and records explicit top reasons. |
| `analysis::domains::provider` | `AnalysisDb` | `derive_abstract_domains_with_cache_stats` and `replace_abstract_domain_facts` | WIRED | Provider stores normalized observations/events and metadata. |
| `analysis_kernel::run` | `polint.abstract_domains` | provider invocation after calls before metrics | WIRED | Provider-order tests and eval fixture show expected ordering. |
| `analysis_kernel::validation` | `analysis::domains::validate` | `validate_abstract_domains` hook | WIRED | Validation runs before generic metadata checks and fails closed. |
| `analysis_kernel::debug` | eval observation | `metadata_debug_json_for_test()["abstract_domains"]` | WIRED | Eval fixture consumes compact debug rows only. |
| public CLI/SDK boundary | private domains | integration source/output scans | WIRED | No unsupported abstract-domain API or CLI contract is promoted. |

### Data-Flow Trace

| Stage | Data | Source | Produces Real Data | Status |
|---|---|---|---|---|
| Domain transfer | `ProductState` updates | MIR operations, CFG edges, call/unresolved rows | Yes | FLOWING |
| Local solver | `DomainResults` | Sorted per-function MIR/CFG/calls inputs | Yes | FLOWING |
| Provider | `DomainOutput` | Solver results plus provider policy/cache inputs | Yes | FLOWING |
| Storage | `DomainStore` | Normalized observation/event rows | Yes | FLOWING |
| Validation/debug | diagnostics and debug JSON | Stored domain rows and metadata | Yes | FLOWING |
| Eval | observed facts/invariants | Test-facing abstract-domain debug JSON | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Focused branch predicate code-review fix | `cargo test -p polint merge_language_outputs_offsets_branch_predicate_place_references --locked` | passed | PASS |
| Domain internals and law/solver/provider tests | `cargo test -p polint analysis::domains --locked` | passed | PASS |
| Public abstract-domain no-leak proof | `cargo test -p polint abstract_domain_internals_stay_private --test cli --locked` | passed | PASS |
| Domain validation | `cargo test -p polint analysis_kernel::validation::abstract_domains --locked` | passed | PASS |
| Full workspace regression | `cargo test --locked` | passed: 896 lib tests, 121 CLI tests, bench tests, macro tests, doctests | PASS |
| Schema drift | `node $HOME/.codex/get-shit-done/bin/gsd-tools.cjs verify schema-drift 31` | `drift_detected: false`, `blocking: false` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-INT-01 | 31-01 through 31-05 | polint has a P0 abstract-domain kernel with lattice/transfer traits, deterministic worklist solving, and first local domains for reachability, nilness/nullishness, truthiness, constants, simple strings, and cheap initializedness. | SATISFIED | Private domain contracts, P0 slots, solver, transfer, result cursor, provider/store/cache identity, validation/debug, eval fixture, and public no-leak tests are present and passing. |

No orphaned Phase 31 requirements found. Phase 32 summary kernel and later query/extension/promotion work remain intentionally out of scope.

### Code Review Closure

Phase 31 code review found four warnings and all were fixed:

- WR-01 canonicalized top-reason joins.
- WR-02 cleared stale assignment/copy target facts.
- WR-03 avoided treating predicate IDs as place IDs.
- WR-04 hid private domain validation internals from public diagnostics.

A focused rerun warning about branch `predicate_place` offsets during semantic MIR provider merge was also fixed. Final focused review status is clean.

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|---|---|---|---|
| `crates/polint/src/core/mod.rs` | Existing dead-code warning for internal `AnalysisDb` accessor methods | Info | Full regression and focused review passed; this is existing private substrate warning and not a Phase 31 blocker. |

### Human Verification Required

None. This phase is internal Rust analysis behavior with deterministic tests, eval fixtures, code review, schema drift, full regression, and source/output public-boundary checks.

### Gaps Summary

No blocking gaps found. Phase 31 delivers the private P0 abstract-domain kernel, deterministic local solving over MIR/CFG/calls, explicit uncertainty states, provider/cache/metadata wiring, validation/debug/eval proof, and public no-leak guarantees. Summary kernels, demand queries, extensions, refined call graphs, and public SDK promotion remain later phases by roadmap design.

---

_Verified: 2026-05-21T13:26:09Z_
_Verifier: Codex_
