//! Private unified solver core (GRAPH-03; provider `polint.solver` lands in Plan 03).
//!
//! This module is the structural heart of v1.3's graph engine: a single shared,
//! deterministic constraint solver that consumes the Phase 44 frontend constraint
//! vocabulary (`crate::analysis::semantic_graph::constraints::ConstraintKind`) and
//! derives edges with explicit budgets, per-language [`policy::SolverPolicy`]
//! scaffolding, and (in Plan 02) full provenance. v1.2's `points_to::solver`
//! fixpoint is folded in **by composition** as the first sub-domain policy
//! (D-03) — its observable behavior, snapshots, and determinism fixtures stay
//! byte-identical. Every type here is `pub(crate)` (D-01/D-16); nothing is
//! promoted to the public SDK (the Phase 42 public-surface-leak gate must stay
//! green).
//!
//! ## D-04 naming-collision guard (MANDATORY)
//!
//! The unified `analysis::solver` core sits **above** the points-to sub-domain.
//! Do not conflate the two vocabularies:
//!
//! - The unified core consumes the GRAPH-02
//!   `semantic_graph::constraints::ConstraintKind` vocabulary (copy/alloc/field/
//!   call/model/type edges over `SemanticNodeId`s) and emits derived edges with
//!   provenance. It owns the worklist/budget/policy abstraction.
//! - The `points_to` sub-domain keeps its own internal language —
//!   `crate::analysis::points_to::facts::PointsToConstraintKind` over
//!   `PtVarId`/`ObjectTokenId`s — unchanged. The fold (D-03) registers the
//!   points-to fixpoint as one [`policy::SolverPolicy`] implementation; it does
//!   NOT merge, rename, or delete the points-to enums.
//!
//! The unified [`budget::SolverBudget`]/[`budget::BudgetStatus`] generalize the
//! points-to budget shapes; `PointsToBudget`/`PointsToBudgetStatus` remain a
//! sub-domain projection of the unified types (D-05), so points-to fixtures stay
//! byte-identical.
//!
//! ## D-11 dependency contract (MANDATORY)
//!
//! The solver core honors a strict, cycle-free dependency contract:
//!
//! - **Closed input set.** A run consumes a fixed snapshot of already-trusted
//!   upstream facts/constraints produced earlier in the same `polint check` run.
//!   The solver never re-reads mutated upstream state mid-run; function/procedure
//!   summaries are an *input* to the solver, never re-fed into the same fixpoint
//!   as they are produced (no solver↔summary loop).
//! - **Single fixpoint per run.** One deterministic [`engine`] worklist drain to
//!   convergence per run; accumulation is `BTreeMap`/`BTreeSet`-ordered and dense
//!   IDs are assigned only after a stable-key sort (D-02), which is what makes the
//!   output byte-stable.
//! - **Bounded outer iterations.** An explicit cap is enforced via
//!   [`budget::SolverBudget`]; exhaustion is surfaced honestly as
//!   [`budget::BudgetStatus::BudgetExceeded`] (never a silent drop, never an
//!   unbounded loop). Derived edges reject the exact precision tier (D-06).

pub(crate) mod budget;
pub(crate) mod cache_key;
pub(crate) mod engine;
pub(crate) mod facts;
pub(crate) mod policy;
pub(crate) mod provenance;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod validate;
