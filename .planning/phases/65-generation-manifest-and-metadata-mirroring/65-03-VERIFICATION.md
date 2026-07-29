---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 03
scope: r3-only
verified: 2026-07-29T11:25:17Z
status: gaps_found
score: "5/7 must-haves verified"
phase_65_complete: false
requirements_completed: []
decision_coverage:
  verified: 24
  total: 27
  gaps: [D-16, D-23, D-25]
security:
  standard: ASVS-L1
  high_risk_gaps: 0
re_verification: false
---

# Phase 65 Plan 03 (R3): Provider Closure Verification Report

**Scope:** Plan 03 / restart slice R3 only

**Status:** gaps_found

**Score:** 5/7 R3 must-have truths verified

This report does **not** verify or complete Phase 65. R4-R6 and
STORE-04/STORE-05/META-01/META-04 remain open. No roadmap, state, requirement,
or completion status was changed.

## Result

R3 establishes the closed provider-outcome model, dependency closure,
post-validation capability blockers, private production dispatch, and
semantic/cache-telemetry separation. The implementation stays exactly within
its declared product/test budget and the focused checks pass well below sixty
seconds.

Two plan-level gaps prevent a pass:

1. D-16 requires fact-family ownership on attributable structured validation
   issues, but every production `ValidationIssue` sets `fact_family: None`.
2. D-23/D-25 require the representative cold/warm proof to compare effective
   capabilities, rule-dispatch decisions/counters, policy answers/order, and
   derived exit behavior. The committed cold/warm test compares provider
   outcomes, runtime blocker IDs, and kernel diagnostics only.

## Observable Truths

| # | R3 must-have truth | Status | Evidence |
|---|---|---|---|
| 1 | R3-only scope, locked R1/R2, three tasks, exact product/test budget, no durable/persisted families, and no completion bookkeeping | ✓ VERIFIED | The implementation range contains exactly 14 declared product/test files and +2,500/-733 lines. No CI, store schema, roadmap, state, or requirement file changed. Plan/summary retain `requirements: []` and R4-R6 remain open. |
| 2 | One deterministic closed outcome per static manifest, authenticated identity only on success, typed non-success state, separate telemetry, existing fatal-error behavior | ✓ VERIFIED | `ProviderOutcomeStatus` has the six locked variants; sealing checks manifest completeness/order; identity is success-only; typed stage/reason/blockers are distinct from `ProviderTelemetry`. Outcome codec/matrix tests pass. |
| 3 | Exact current hard dependencies, plan-first gating, pre-run and post-validation fixed-point blocking, independent progress, and no absent-digest trust substitute | ✓ VERIFIED | The crate-private dependency table is exercised by synthetic A/B/C/D/E closure tests and explicit kernel scheduling checks. Failure ledger checks precede identity creation, and final validation downgrades feed deterministic closure. |
| 4 | One deterministic validation report with provider **and fact-family** ownership when attributable; one-way diagnostic rendering; owned/global fail-closed downgrade | ✗ GAP | Provider ownership and global fallback work, but `ValidationIssue::from_rendered` always writes `fact_family: None`; no production assignment or consumer exists. The field is explicitly covered by `#[expect(dead_code)]`. D-16 is therefore incomplete. |
| 5 | Effective hard-capability revocation and deterministic pre-`RuleCtx` rule skipping, unchanged public contracts, store maintenance after sealing | ✓ VERIFIED | Kernel blockers are derived after validation/sealing, production runner dispatch forwards the private blocker set, core tests prove blocked rules stay uncalled while unrelated rules run, and store maintenance remains after semantic finalization. |
| 6 | Cold/warm semantic identity across outcomes, capabilities, dispatch, policy diagnostics/order/exit; invalid cache rejection; focused full proof | ✗ GAP | Cache corruption/recompute, warning telemetry, outcome equality, blocker equality, and kernel-diagnostic equality are tested. The cold/warm pair does not compare `capability_support`, dispatch counters/decisions, policy answers, or derived exit semantics, despite the explicit Task 3 and D-23/D-25 proof contract. |
| 7 | Public/store parity and privacy, parallel-safe sub-60-second tests, CI and deferred sub-five-minute redesign untouched | ✓ VERIFIED | Public leak and store parity tests pass; new implementation names remain crate-private; no product test uses sleeps, environment mutation, or global serialization; every required focused test finished in under 3 seconds; `.github/workflows/ci.yml` is unchanged. |

## Required Artifacts

| Artifact | Status | Verification |
|---|---|---|
| `crates/polint/src/analysis_kernel/outcome.rs` | ✓ VERIFIED | Substantive closed status, identity, failure, dependency, tracker, sealing, effective-capability, and blocker implementation; 6 focused tests pass. |
| `crates/polint/src/analysis_kernel/validation.rs` | ⚠ PARTIAL | `ValidationReport`, deterministic rendering, provider ownership, and global fallback are substantive and wired, but the promised fact-family ownership is never populated. |
| `crates/polint/src/analysis_kernel/incremental/run_report.rs` | ✓ VERIFIED | One manifest-ordered outcome row per provider and separate telemetry; 5 focused tests pass. |
| `crates/polint/src/analysis_kernel/incremental/mod.rs` | ✓ VERIFIED | Curated crate-private outcome and telemetry re-exports. |
| `crates/polint/src/analysis_kernel/mod.rs` | ✓ VERIFIED | Plan-first scheduling, failure ledger, post-validation sealing, blocker finalization, and post-seal store maintenance are wired. |
| `crates/polint/src/core/mod.rs` | ✓ VERIFIED | Fallible fact replacement produces typed provider failure signals; private runtime blockers are checked before rule invocation. |
| `crates/polint/tests/public_surface_leak.rs` | ✓ VERIFIED | Outcome status, output identity, failure signal, validation report, and validation issue markers are scanned; the supported-surface test passes. |

Artifact existence/marker validation passed 7/7. Semantic inspection leaves
the validation artifact partial, so 6/7 fully satisfy their stated contract.

## Key-Link Verification

| From | To | Link | Status |
|---|---|---|---|
| `analysis_kernel/mod.rs` | `analysis_kernel/outcome.rs` | Explicit calls consult/record the tracker; sealing occurs after validation | ✓ WIRED |
| `core/mod.rs` | `analysis_kernel/mod.rs` | Replacement and Go setup/client failures enter the typed failure ledger | ✓ WIRED |
| `analysis_kernel/validation.rs` | `analysis_kernel/outcome.rs` | Provider/global downgrades feed manifest-order fixed-point sealing | ✓ WIRED |
| `analysis_kernel/outcome.rs` | `runner/mod.rs` | Effective closure produces sorted rule blockers consumed by production dispatch | ✓ WIRED |
| `incremental/run_report.rs` | `incremental/stats.rs` | Semantic outcomes remain separate from telemetry; only telemetry aggregates cache counters | ✓ WIRED |

All 5 declared key links are structurally present and exercised. The D-16 gap
is inside the validation artifact's attribution payload, not a missing
validation-to-sealing link.

## Locked Decision Coverage

| Decision | Status | Evidence |
|---|---|---|
| D-01 | ✓ | Only R3 product/test work is in the implementation range; R1/R2 are unchanged dependencies. |
| D-02 | ✓ | Three tasks, 14 product/test files, exactly 2,500 additions, zero durable schema families, zero persisted provider families. |
| D-03 | ✓ | No provider-key migration, dependency-index redesign, persistence, Go certification, semantic-ID repair, or public expansion. |
| D-04 | ✓ | No phase/requirement completion; `requirements: []`; R4-R6 remain next. |
| D-05 | ✓ | Sealed report covers every static manifest exactly once in manifest order, including planned absence. |
| D-06 | ✓ | Closed six-state enum; no optional digest, diagnostic string, or cache counter is used as final status. |
| D-07 | ✓ | Only validated provisional success seals as `Succeeded` with identity. |
| D-08 | ✓ | Non-success states carry typed stage/reason and sorted exact blockers; identity is absent. |
| D-09 | ✓ | Pre-establishment fatal errors remain existing kernel errors; no fabricated report/panic framework. |
| D-10 | ✓ | Cache telemetry and open display validation remain outside semantic truth. |
| D-11 | ✓ | Small crate-private direct hard-dependency table; no durable graph or broad `DependencyIndex` promotion. |
| D-12 | ✓ | Scheduled consumers check hard-producer usability; independent branches continue. |
| D-13 | ✓ | Plan gating precedes dependency blocking; omitted providers seal `PlannedAbsent`. |
| D-14 | ✓ | Missing/failing producers are typed; no `Digest::absent` substitution was introduced for provider trust. |
| D-15 | ✓ | No degraded/implicit edge was invented. |
| D-16 | ✗ | Structured provider ownership exists, but fact-family ownership is a permanently `None` field in production. |
| D-17 | ✓ | Owned issues downgrade implicated providers; unowned issues mark global and downgrade all provisional successes. |
| D-18 | ✓ | Effective hard capabilities require final succeeded provider closure. |
| D-19 | ✓ | Planning/setup lower bounds remain authoritative. |
| D-20 | ✓ | Affected rules are blocked before `RuleCtx`; unrelated rules continue; capability diagnostics are deterministic. |
| D-21 | ✓ | Machinery is crate-private; no public support variant, SDK/CLI/JSON/diagnostic/exit contract was added. |
| D-22 | ✓ | Outcomes and blockers seal before output/report construction; store maintenance follows. |
| D-23 | ✗ NOT FULLY PROVEN | The cold/warm test omits effective-capability, dispatch, policy, ordering, and exit comparisons. No contradictory behavior was observed, but the locked parity boundary lacks its required behavior-level proof. |
| D-24 | ✓ | Corrupt cached payload is evicted/recomputed; write failure remains telemetry while semantic outcomes stay successful/planned-absent. |
| D-25 | ✗ INCOMPLETE PROOF | Codec/matrix, dependency chain, failures, ordering, dispatch, and cold/warm pieces exist, but the cold/warm pair does not carry dispatch/policy/exit assertions through the same semantic projection. |
| D-26 | ✓ | Public bytes/exit store-mode parity passes; private outcome/validation markers are leak-scanned; store state does not alter provider truth. |
| D-27 | ✓ | All focused tests are under 60 seconds, isolated, and parallel-safe; CI/timeouts were untouched. |

Decision coverage is 24/27. D-23 describes an unproven parity boundary rather
than an observed cold/warm mismatch; D-25 is the corresponding missing focused
proof.

## Behavioral Verification

All commands were run from the repository root at verification HEAD
`bbdedff7`. Times are wall-clock `real` measurements from individual focused
commands.

| Command / focus | Result | Time |
|---|---:|---:|
| `cargo test -p polint --lib analysis_kernel::outcome::tests --locked` | 6 passed | 1.01 s |
| `cargo test -p polint --lib eval::performance::tests --locked` | 6 passed | 0.15 s |
| `cargo test -p polint --lib analysis_kernel::validation::tests --locked` | 9 passed | 0.25 s |
| `cargo test -p polint --lib analysis_kernel::tests::provider_outcomes --locked` | 3 passed | 0.17 s |
| Core runtime-provider blocker regression | 1 passed | 0.11 s |
| Production mixed blocked/unrelated dispatch regression | 1 passed | 0.18 s |
| Events-only plan regression | 1 passed | 0.13 s |
| Deep/refined calls regression | 1 passed | 0.16 s |
| Events policy query regression | 1 passed | 0.12 s |
| Refined validation regression | 1 passed | 0.13 s |
| Validation ownership/render regression | 1 passed | 0.11 s |
| Public-surface leak regression | 1 passed | 2.32 s |
| Semantic-store byte/exit parity regression | 1 passed | 0.46 s |
| Run-report tests | 5 passed | 0.64 s |
| Incremental stats tests | 3 passed | 0.17 s |
| Go semantic provider tests | 6 passed | 0.17 s |
| Symbol-graph cache restore tests | 2 passed | 0.61 s |
| Semantic-graph snapshot tests | 9 passed | 2.47 s |
| `cargo fmt --all -- --check` | passed | 1.35 s |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | passed | 0.34 s |
| `cargo check --workspace --all-features --locked` | passed | 0.15 s |
| `git diff --check c453748c..f86dab67` | passed | 0.04 s |

No required individual test approached the 60-second ceiling. The one
pre-existing ignored synthetic performance benchmark was not introduced by R3
and is not used as evidence here. R3's new correctness tests are active and use
isolated temporary repositories/caches without sleeps, environment mutation,
network access, or process-global serialization.

### Test-quality audit

- Outcome and fixed-point tests assert exact enum state, stage/reason, identity,
  manifest order, and blocker order rather than only non-emptiness.
- Dispatch tests use atomic counters and prove blocked rules remain uncalled
  while unrelated rules execute in sequential/parallel core paths and the
  production adapter.
- Cache corruption is performed against an isolated real layer-cache blob and
  asserts invalid eviction plus recomputation.
- Store parity renders real JSON and derives an exit byte across enabled,
  disabled, corrupt, future, invalid, and busy states.
- No expected output is generated by the implementation path under test.
- The cold/warm test-quality gap is material: lines 3358-3366 of
  `analysis_kernel/mod.rs` compare outcomes, runtime blocker IDs, diagnostics,
  and differing telemetry, but omit the rest of the D-23 semantic projection.

## Product/Test Scope and Budget

Implementation range: `c453748c..f86dab67`.

| Product/test file | Added | Deleted |
|---|---:|---:|
| `analysis_kernel/incremental/mod.rs` | 2 | 2 |
| `analysis_kernel/incremental/run_report.rs` | 90 | 63 |
| `analysis_kernel/incremental/stats.rs` | 9 | 63 |
| `analysis_kernel/mod.rs` | 946 | 471 |
| `analysis_kernel/outcome.rs` | 781 | 0 |
| `analysis_kernel/validation.rs` | 127 | 27 |
| `core/mod.rs` | 151 | 14 |
| `eval/observed.rs` | 40 | 34 |
| `eval/performance.rs` | 161 | 42 |
| `eval/semantic_graph_snapshot.rs` | 12 | 3 |
| `go/semantic/provider.rs` | 43 | 1 |
| `runner/mod.rs` | 108 | 5 |
| `symbol_graph/mod.rs` | 23 | 8 |
| `tests/public_surface_leak.rs` | 7 | 0 |
| **Total** | **2,500** | **733** |

This is exactly 14 product/test files, all declared by the plan, within the
15-file cap and exactly at the 2,500-addition cap. `.github/workflows/ci.yml`,
`.planning/STATE.md`, `.planning/ROADMAP.md`, and
`.planning/REQUIREMENTS.md` are unchanged in the implementation range. No
public SDK/CLI/docs, durable schema, migration, store publication, or
requirement-tracking expansion occurred.

## Security and Abuse-Case Review

No high-risk ASVS L1 issue was found in R3's private trust boundary.

| Threat | Result |
|---|---|
| Cache hit or forged digest certifies success | ✓ Typed decode/validation and failure-ledger checks precede success identity; corruption recomputes. |
| Failed producer is consumed as an empty universe | ✓ Scheduled hard consumers block; post-validation fixed-point closure revokes already-provisional consumers. |
| A blocked rule accesses partial facts | ✓ Runtime blocker set is consumed before `RuleCtx`; atomic-counter tests prove non-execution. |
| Rendered diagnostic text controls trust | ✓ Downgrades use structured provider IDs/global state; diagnostics render one-way from issues. |
| Unowned validation issue certifies a partial universe | ✓ Global fallback downgrades every provisional success. |
| Store/cache telemetry mutates semantic truth | ✓ Telemetry is separate and store maintenance occurs after sealing; store-mode parity passes. |
| Private trust vocabulary leaks into supported contracts | ✓ Visibility inspection, public leak test, and unchanged SDK/CLI/JSON surfaces pass. |

The missing fact-family attribution weakens structured traceability but does not
currently create a fail-open path because provider ownership and the global
fallback still drive downgrades. The incomplete cold/warm test leaves part of
the parity invariant unproven; source inspection found no direct cache-driven
dispatch or policy branch, but that is not a substitute for the locked
behavioral proof.

## Anti-Patterns

One material placeholder was found:

- `ValidationIssue.fact_family` is declared under
  `#[expect(dead_code, reason = "structured attribution")]`, initialized only
  to `None`, and never read by production code. This is the D-16 gap, not merely
  harmless forward scaffolding, because the plan makes fact-family ownership an
  observable R3 contract.

No added `TODO`, `FIXME`, `XXX`, `todo!`, `unimplemented!`, shipped
phase-history comment, sleep, environment mutation, or serialization mutex was
found in the R3 diff.

## Requirements

| Requirement | R3 status |
|---|---|
| STORE-04 | OPEN — R3 contributes private provider truth only |
| STORE-05 | OPEN — R3 contributes authenticated in-memory identity only |
| META-01 | OPEN — R3 contributes validation ownership/closure only |
| META-04 | OPEN — R3 contributes cache-truth separation only |

R3 completes no requirement. This report intentionally does not edit or
certify requirement tracking.

## Human Verification

None required. R3 is a private infrastructure slice whose observable contract
can be checked through source inspection, deterministic focused tests, public
boundary probes, and byte/exit parity fixtures.

## Gap Remediation

### 1. Populate structured fact-family ownership

Change validation collection so an attributable issue receives its
`FactFamily` from the validator's explicit family or authenticated
`FactMeta.family`; do not parse family/provider names back out of rendered
diagnostic text. Add focused assertions that a known malformed fact produces
the expected `Some(FactFamily::...)`, provider ownership remains sorted, and
truly global issues retain `None` plus global fail-closed downgrade.

### 2. Complete the representative cold/warm semantic projection

Extend the isolated cold/warm regression through production dispatch. Compare
final outcomes/identities/blockers, `capability_support`, sorted
validation/capability diagnostics, affected and unaffected rule counters,
policy diagnostics/answers/order, and the derived exit byte. Only provider
telemetry should differ. Retain the existing corrupt-payload and write-warning
branches.

The R3 addition budget is already exactly saturated. Any remediation must
delete/rebalance implementation lines or deliberately revise the locked budget
through the owning GSD workflow; this verifier does not authorize a silent
scope increase.

---

_Verified: 2026-07-29_

_Verifier: Codex, R3-only GSD verification_
