# Phase 57: Control-Flow Guard and Lifecycle Queries - Context

**Gathered:** 2026-06-20
**Status:** Ready for planning
**Mode:** Autonomous, inline

<domain>
## Phase Boundary

Phase 57 promotes `ControlFlow<'_>` from preview vocabulary to a backed policy
query surface for two useful rule families:

- `missing_guard(GuardQuery)` for "guard call must happen before sensitive call"
  policies.
- `missing_cleanup(LifecycleQuery)` for "start/acquire call must be followed by
  cleanup/release call" policies.

The first supported scope is same-function call-event ordering. It should reuse
the Phase 56 public pattern/query vocabulary and private provider facts:
refined calls, direct call sites, MIR operation identity, and CFG operation
order where available. It must not expose raw CFG nodes, dominance,
postdominance, MIR IDs, call graph IDs, or provider rows to rule authors.

</domain>

<decisions>
## Implementation Decisions

- Keep the public API simple: rule authors still write `ControlFlow<'_>` plus
  `GuardQuery` or `LifecycleQuery`; no alternate DSL, callbacks, closures,
  regexes, or raw graph selectors.
- Implement only call-event-backed control-flow queries in this phase.
  `EventPattern::write_field` remains vocabulary-only until a stable write-event
  projection exists.
- Treat same-function ordering as useful but conservative policy evidence.
  Diagnostics should use `PolicyStatus::Heuristic` and
  `PolicyPrecision::Conservative`, even when the underlying call target is
  resolved.
- Use `max_paths` as the returned-violation cap and surface budget truncation in
  diagnostic evidence.
- Interpret `max_depth` honestly: Phase 57 evaluates same-function depth only.
  Bounded interprocedural guard/lifecycle search is deferred rather than faked.
- Keep `DataFlow<'_>` fail-closed for Phase 58.

</decisions>

<code_context>
## Existing Code Insights

- `crates/polint/src/policy_queries.rs` already hosts provider-backed Phase 56
  `Events` and `Calls` query logic behind thin SDK wrappers.
- `sdk/facts.rs` currently has `ControlFlow<'_>` methods that call
  `preview_query_unavailable`; those wrappers should become thin calls into the
  private query module.
- `sdk/policy.rs` exposes public query/pattern structs but only gives private
  accessors to `EventPattern`; `GuardPattern` needs matching private accessors.
- `analysis_plan.rs` still marks `control_flow` unsupported; promoting Phase 57
  requires moving it to supported while leaving `dataflow` unsupported.
- `analysis_kernel/mod.rs` gates the semantic pipeline on capability names and
  already includes `events` and `calls`; `control_flow` should join that trigger
  list.
- `docs/facts/control-flow.md` and `.agents/skills/polint/SKILL.md` still
  describe `ControlFlow<'_>` as fail-closed and need to be updated after code
  behavior lands.

</code_context>

<specifics>
## Specific Ideas

- Build an internal ordered call-event projection from refined call edges joined
  to call sites. Include function, file/range, target candidates, stable target
  label, status/precision/confidence, and an order source.
- Prefer CFG node `operation_ordinal` for call-site order when the call site's
  MIR operation has a CFG node. Fall back to source span and stable key when CFG
  order is unavailable.
- `missing_guard` reports a sensitive call event when no earlier same-function
  guard call matches `GuardPattern::call_any`.
- `missing_cleanup` reports a start/acquire call event when no later
  same-function cleanup call matches the cleanup `EventPattern`.
- Evidence should include target, required guard or cleanup, control scope,
  order source, depth, uncovered path, and budget status when truncated.

</specifics>

<deferred>
## Deferred Ideas

- Exact dominance/postdominance and per-exit cleanup proof.
- Bounded interprocedural guard and lifecycle search.
- Field/property write events such as `account.balance`.
- Resource identity pairing across different acquired values.
- Public raw `Cfg<'_>` or `CallGraph<'_>` graph traversal.

</deferred>
