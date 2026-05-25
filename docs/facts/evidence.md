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
