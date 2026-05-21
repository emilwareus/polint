# Phase 31: P0 Abstract-Domain Kernel - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-05-21
**Phase:** 31-p0-abstract-domain-kernel
**Mode:** `--auto`
**Areas discussed:** Kernel boundary, Lattice/product shape, Solver scope, Truthfulness, Validation/debug/eval/cache

---

## Kernel Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Private kernel substrate | Keep abstract-domain contracts crate-private and consume existing MIR/CFG/place/call rows. | yes |
| Public SDK facts now | Promote domain facts or query views immediately. | |
| Parser/tool-native analysis | Run domains directly on parser ASTs or raw language-tool objects. | |

**Auto choice:** Private kernel substrate.
**Notes:** Matches v1.2 public API discipline and Phase 28-30 private-provider pattern.

---

## Lattice And Product Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Small law-tested core domains | Add bottom/top/order/join/widen/digest/transfer traits plus P0 local slots. | yes |
| Broad relational engine | Implement symbolic execution, Datalog, octagons/polyhedra, or full points-to first. | |
| Single hardcoded analysis | Build one analysis instead of reusable domain kernel. | |

**Auto choice:** Small law-tested core domains.
**Notes:** Research recommends a reduced product and a precision ladder rather than a universal analyzer.

---

## Solver Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Local deterministic solver | Solve per-function CFG/MIR with stable order, bounded iteration, widening fuel, and conservative call handling. | yes |
| Interprocedural summaries now | Add summary projection/application and SCC scheduling in this phase. | |
| Whole-program fixed point now | Attempt global analysis over all calls and dynamic dispatch. | |

**Auto choice:** Local deterministic solver.
**Notes:** Summary kernel and demand-query scheduling are already scoped to later phases.

---

## Truthfulness And Unknowns

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit top/unknown statuses | Surface unsupported semantics, unresolved calls, dynamic writes, setup gaps, budget exhaustion, and widening as conservative statuses. | yes |
| Silent fallback to certainty | Drop unsupported semantics or pretend exactness. | |
| Diagnostic-first precision | Optimize for diagnostics before the facts can justify them. | |

**Auto choice:** Explicit top/unknown statuses.
**Notes:** Aligns with project truthfulness constraints and SAE-INT-01 validation needs.

---

## Validation, Debug, Eval, And Cache

| Option | Description | Selected |
|--------|-------------|----------|
| Full private proof chain | Add law tests, transfer monotonicity, deterministic debug/eval, cache identity, and public no-leak proof. | yes |
| Unit tests only | Skip eval/cache/no-leak proof until later. | |
| Public docs first | Add public docs/facts before promotion gates. | |

**Auto choice:** Full private proof chain.
**Notes:** Follows the established Phase 20-30 GSD standard for new internal analysis families.

---

## the agent's Discretion

- Exact Rust module names and plan boundaries.
- Whether to create one provider immediately or stage contracts/solver/provider/debug/eval across separate plans.
- Which "where practical" P0 domain slices are honest with current MIR/CFG inputs.

## Deferred Ideas

- Interprocedural summaries and summary SCC scheduling.
- Extension-authored domains.
- Framework dispatch and trust-boundary facts.
- Broad type/value/alias substrate, refined call graph, data-flow, slicing, benchmark gates, and public SDK promotion.
