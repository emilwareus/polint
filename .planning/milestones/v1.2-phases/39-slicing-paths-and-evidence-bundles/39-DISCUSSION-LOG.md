# Phase 39: Slicing, Paths, and Evidence Bundles - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-25T13:45:51Z
**Phase:** 39-slicing-paths-and-evidence-bundles
**Mode:** `$gsd-discuss-phase 39 --auto`
**Areas discussed:** Evidence bundle contract, Slice modes and defaults, Path ranking and context, Summary expansion and rendering, Extension evidence merge, Validation and eval proof

---

## Evidence Bundle Contract

| Option | Description | Selected |
|--------|-------------|----------|
| Internal bundle plus scalar compatibility | Add private structured evidence bundles while preserving existing scalar diagnostic evidence. | ✓ |
| Replace scalar evidence now | Remove or reshape the existing evidence contract around structured bundles. | |
| Keep evidence only in debug JSON | Avoid diagnostic/report integration in this phase. | |

**Auto choice:** Internal bundle plus scalar compatibility.
**Notes:** Preserves current report compatibility while giving Phase 39 a real internal evidence substrate.

---

## Slice Modes and Defaults

| Option | Description | Selected |
|--------|-------------|----------|
| Thin slices by default | Use compact producer-focused slices for diagnostics; keep full modes for debug/eval. | ✓ |
| Full slices by default | Show broad data/control/call/model evidence by default. | |
| Path-only explanations | Skip slice queries and only render source-to-sink paths. | |

**Auto choice:** Thin slices by default.
**Notes:** Matches research guidance that full slices are too large for primary diagnostic explanations.

---

## Path Ranking and Context

| Option | Description | Selected |
|--------|-------------|----------|
| Bounded deterministic ranked paths | Use capped k-path extraction, stable tie-breaks, explicit budgets, and call-site stack matching. | ✓ |
| All feasible paths | Attempt broad path enumeration. | |
| Shortest path only | Return only one shortest path without ranking alternatives. | |

**Auto choice:** Bounded deterministic ranked paths.
**Notes:** Avoids unbounded enumeration and preserves interprocedural correctness through call/return context.

---

## Summary Expansion and Rendering

| Option | Description | Selected |
|--------|-------------|----------|
| Compressed summaries with expansion handles | Render summary edges compactly and expose expansion keys/opaque reasons. | ✓ |
| Always expand summaries eagerly | Expand all summary evidence inline. | |
| Hide summaries from evidence | Omit summary edges from user-facing evidence. | |

**Auto choice:** Compressed summaries with expansion handles.
**Notes:** Keeps evidence bounded while allowing agents/debug paths to inspect expansion when available.

---

## Extension Evidence Merge

| Option | Description | Selected |
|--------|-------------|----------|
| Additive validation-gated evidence | Extensions can add candidate/accepted evidence with precision ceilings and validation. | ✓ |
| Allow extensions to override native evidence | Let extensions suppress or strengthen native paths directly. | |
| Ignore extensions in evidence | Exclude repo-local extensions from evidence. | |

**Auto choice:** Additive validation-gated evidence.
**Notes:** Preserves polint's repo-local extension value while preventing untrusted suppression or exactness claims.

---

## Validation and Eval Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Native fixture and renderer proof | Prove local dependence, thin/full slices, paths, context, summaries, extension evidence, determinism, and public no-leak coverage. | ✓ |
| External benchmarks first | Start with external suites before native evidence fixtures. | |
| Unit tests only | Rely on unit-level proof without native eval fixtures. | |

**Auto choice:** Native fixture and renderer proof.
**Notes:** External adapters belong to Phase 40; Phase 39 must first prove the internal evidence contract with native fixtures.

---

## The Agent's Discretion

- Exact module layout and enum/type names.
- Whether evidence bundles are stored in `AnalysisDb` or materialized per diagnostic/query.
- Exact plan split across evidence contracts, slicing, paths, rendering, validation/debug/eval, and no-leak proof.
- How narrow to keep first interprocedural support as long as call-site stack matching and summary compression are proven.

## Deferred Ideas

- Public SDK evidence/path/slice views remain Phase 41.
- External benchmark adapters and promotion reports remain Phase 40.
- Full all-pairs path materialization and broad IFDS/IDE tabulation remain future work.
