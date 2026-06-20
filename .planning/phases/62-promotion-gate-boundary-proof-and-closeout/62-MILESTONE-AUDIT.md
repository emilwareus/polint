# Phase 62 Milestone Audit: v1.4 Policy Query Surface

**Date:** 2026-06-20
**Milestone:** v1.4 Policy Query Surface
**Verdict:** Ready for local review as a preview SDK surface.

## Preview APIs Ready

The v1.4 surface gives rule authors one public way to write advanced policies:

1. Request a typed preview view in a `#[polint::rule]` function signature.
2. Construct one plain query object with typed pattern structs.
3. Run one method on that view.
4. Report returned `PolicyViolation` values through `violation.diagnostic(...)`.

Ready preview views:

- `Events<'_>::matching(EventPattern::call(...))` for semantic call-event matches.
- `Calls<'_>::forbidden_reachable(ReachQuery)` for reachable forbidden-call policies.
- `ControlFlow<'_>::missing_guard(GuardQuery)` for same-function guard-before-call policies.
- `ControlFlow<'_>::missing_cleanup(LifecycleQuery)` for same-function acquire/cleanup policies.
- `DataFlow<'_>::forbidden(FlowQuery)` for bounded source/sink/barrier policies.

Backed pattern/query coverage:

- Call events through `EventPattern::call`.
- Reach roots and targets through `ReachQuery`.
- Guards through `GuardPattern::call_any`.
- Lifecycle acquire/cleanup calls through `LifecycleQuery`.
- Flow sources through HTTP request sources and secret-like names.
- Flow sinks through exact call sinks and logger sinks.
- Flow barriers through call-based barrier patterns.

The flagship `polint new-rule --template` scaffolds now cover request-to-shell,
secret-to-log, PII-to-analytics, sensitive-write guard, transaction cleanup,
raw reachable API, SSRF, dangerous HTML, unsafe deserialization, and user-file
path policies.

## Reserved Internals

The following remain intentionally unavailable from supported SDK, CLI, runner,
README, generated skill text, and `docs/facts` rule-authoring surfaces:

- Raw CFG views and dominance/postdominance graph APIs.
- Raw call graph traversal and call graph node IDs.
- Raw semantic graph, MIR, data-flow graph, provider rows, dense IDs, and
  solver/provider internals.
- `AnalysisDb` and private provider/cache/evaluation structs.
- Public `Evidence<'_>` view, model-pack authoring surface, provider extension
  SDK, and adaptation/model-pack APIs.

This boundary preserves the product decision from the requirements: v1.4
promotes policy-level questions, not graph-programming APIs.

## Known Preview Limits

- `Events<'_>` is provider-backed for call events. Non-call event families are
  reserved vocabulary until backed facts land.
- `ControlFlow<'_>` guard and cleanup checks are same-function call-event
  checks. Exact resource identity, all-exit cleanup proof, field/property write
  events, and bounded interprocedural control-flow are deferred.
- `DataFlow<'_>` uses bounded private path search. It reports evidence,
  precision, unknowns, and budget state honestly, but it is not a perfect
  whole-program taint engine.
- Barrier semantics are conservative call-path suppression. They do not prove
  arbitrary sanitizer correctness.
- Built-in source/sink categories are intentionally narrow. Broader categories
  such as SQL, raw HTML/JSX, SSRF URL taxonomies, file paths, analytics, PII,
  and outbound network clients are currently template guidance or future
  taxonomy work, not comprehensive primitives.
- Query-result cache identity is represented through query digest evidence and
  analysis/cache inputs. A public query-result cache schema remains deferred.
- All v1.4 query names are preview contracts and should stabilize only after
  external rule usage validates the ergonomics.

## Boundary Proof

Boundary proof is covered by:

- `public_surface_leak` tests for SDK/prelude, CLI/runner/README/docs/skill
  surfaces, including v1.4 reserved/raw internals.
- Phase 61 temp-repo SDK matrix tests that import only
  `polint::sdk::prelude::*`, register through `polint::runner::run_cli`, and
  consume real preview facts.
- Phase 62 deterministic flagship-template test that generates external-shaped
  `.polint/rules` and runs representative templates twice with stable JSON
  case and summary output.
- `docs/facts/policy-queries.md` plus per-view docs that describe preview
  status, syntax, precision, unknowns, budgets, and limits.

## v1.5 Follow-Ups

- Stabilize preview names and evidence schema after external usage.
- Add richer first-class source/sink taxonomy where fixtures prove precision.
- Promote backed write events before adding public write-policy templates.
- Improve cleanup/resource identity and all-exit lifecycle proof.
- Consider a public query-result cache/schema only after deterministic evidence
  shape has settled.
- Add IDE or `polint query` exploration once the Rust rule API has proven the
  right primitive set.
- Keep raw graph/solver APIs private unless a separate milestone proves a
  narrow public need.

