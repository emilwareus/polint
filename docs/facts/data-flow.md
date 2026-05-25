# Data-Flow Facts

Data-flow facts are internal derived facts produced by `polint.data_flow`.
They are not part of the public rule-author SDK yet.

## Families

- `DataFlowNode`: run-local nodes for MIR places, call-boundary nodes, source/sink/model nodes, and synthetic query nodes.
- `DataFlowEdge`: directed value-flow edges between nodes.
- `DataFlowModel`: source, sink, sanitizer, barrier, and TITO models from native recognizers or accepted extension facts.
- `DataFlowBudget`: budget observations for bounded data-flow operations.

## Precision

The provider records precision on every edge and model:

- `Exact`: exact provider evidence.
- `SetupAware`: evidence that depends on configured semantic setup.
- `Syntax`: syntax-local evidence.
- `Conservative`: deliberately over-approximated evidence.
- `Heuristic`: model-derived or approximate evidence.
- `Unknown`: unknown precision.

Heuristic rows must be treated as over-approximations. They are useful for
policy checks and review guidance, but they must not be described as complete
program proofs.

## Limits

The first provider slice is intentionally conservative:

- Local nodes mirror available MIR places.
- Direct-call data-flow edges are projected only from resolved refined-call edges.
- Trust boundaries become source models.
- Accepted extension facts may contribute source, sink, sanitizer, barrier, or TITO models when their declared family matches a supported data-flow family.
- Query path search is bounded by depth and path-count budgets and returns a budget status rather than exploring unbounded graphs.

Unsupported or missing semantic setup should be represented as statused facts or
capability diagnostics in later SDK-facing surfaces, not hidden behind placeholder
facts.
