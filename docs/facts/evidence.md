# Diagnostic Evidence

Evidence bundles are currently an internal analysis and reporting feature. They
support bounded diagnostic JSON/SARIF output with path status, precision,
provenance, unknowns, omitted regions, summary expansion handles, replay keys,
and hidden-node counts.

Evidence is not a public SDK fact view in this phase. Repo-local rules should
continue to report scalar diagnostic evidence with `Diagnostic::with_evidence`
and consume only the supported fact views exported by `polint::sdk::prelude`.

The renderer is conservative:

- JSON is the primary structured representation and is versioned as
  `evidence_v1` when present on diagnostics.
- SARIF is a lossy projection through code flows and messages.
- Opaque, unknown, partial, budgeted, and extension-supplied evidence is labeled
  as such and must not be treated as exact coverage.
- Debug and eval rows use relative/stable keys and must not include raw source
  bodies, absolute workspace paths, parser object ids, or timestamps.

Future public evidence fact views should be added only through the SDK contract
with documented limits and heuristic behavior.

## Policy Query Evidence

Preview policy query views return `PolicyViolation` values. When a rule reports
one through `violation.diagnostic(ctx.rule_id(), "...")`, polint emits a stable
scalar evidence header:

See [policy-queries.md](policy-queries.md) for the full policy-query preview
contract, query vocabulary, precision/status vocabulary, and template starters.

- `policy_query`: one of `events.matching`, `calls.forbidden_reachable`,
  `control_flow.missing_guard`, `control_flow.missing_cleanup`, or
  `data_flow.forbidden`.
- `policy_query_version`: the preview policy-query evidence/cache version.
- `query_digest`: a deterministic digest of the query object, including
  patterns, option fields, and the policy-query version.
- `policy_status`: `exact`, `heuristic`, `unknown`, `unsupported`, or
  `budget_exceeded`.
- `policy_precision`: `exact`, `setup_aware`, `syntax`, `conservative`,
  `heuristic`, or `unknown`.

Each query family also emits flat query-specific evidence. For example,
reachable-call policies include `root`, `target`, and `path`; control-flow
policies include `required_guard` or `required_cleanup`; data-flow policies
include `source`, `sink`, `path_status`, and `barrier_status`.

Provider facts are cached by lifecycle/config/model inputs. Policy query
results are evaluated inside rule execution, so runtime query parameters are
not analysis fact-cache keys today. The `query_digest` evidence is the stable
identity for query-result semantics and is the key material a future
query-result cache should use.
