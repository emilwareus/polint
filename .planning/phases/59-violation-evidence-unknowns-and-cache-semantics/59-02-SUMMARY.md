# Phase 59-02 Summary: Policy Unknown and Budget Visibility

## Completed

- Extended public unknown collection for preview policy capabilities:
  - `events`
  - `calls`
  - `control_flow`
  - `dataflow`
- Routed call-oriented policy unknowns through refined-call and call-site status evidence while keeping provider internals private.
- Routed data-flow unknowns through public rows for edge status, search budget observations, and evidence unknown reasons.
- Updated `facts list`, `unknowns`, and `inspect unknowns` behavior so preview policy views can advertise and return actionable unknown rows.

## Notes

- Reserved raw surfaces such as `cfg`, `call_graph`, and low-level graph/debug capabilities remain unsupported public capabilities.
- Empty unknown results for a supported preview policy capability now mean "no public unknown rows for this analysis", not "capability unsupported".

