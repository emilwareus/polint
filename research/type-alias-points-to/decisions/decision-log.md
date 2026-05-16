# Decision Log

## D1: Alias Is A Query Service

Decision: alias analysis should be exposed as a query service over providers, not a materialized global alias graph.

Reason:

- LLVM AliasAnalysis validates provider-stack design.
- Alias answers depend on query needs and available precision.
- `MustAlias` is rare; `MayAlias`/`Unknown` need evidence.

## D2: Type/Value/Place Facts Come First

Decision: implement places, type facts, value facts, and local narrowing before global points-to.

Reason:

- Python and TS/JS tools get most practical precision from these layers.
- Points-to needs stable places and allocation tokens.
- Type facts prune impossible points-to propagation.

## D3: Andersen Is The First Points-To Solver

Decision: use bounded inclusion-based Andersen constraints as the first native points-to provider.

Reason:

- More precise than Steensgaard.
- Maps well to fields, loads/stores, and summaries.
- Well-understood engineering path with bitsets/SCC/deltas.

## D4: No Mandatory Whole-Repo Points-To

Decision: points-to should be requested, cached, and budgeted, not always run globally.

Reason:

- Many rules do not need it.
- Worst-case cost can be high.
- Query-driven precision fits the analysis kernel and SDK.

## D5: Extensions Can Add Precision But Not Hide Uncertainty

Decision: extension providers can add type/value/summary/points-to/alias facts, but cannot silently erase native unknowns or contradict exact facts.

Reason:

- Agent-authored extensions are the product differentiator.
- Unvalidated replacement facts create false confidence.
- Provenance and conflict diagnostics are mandatory for trust.

## D6: Go Tools Are Oracles, Not Runtime Dependencies

Decision: use `go/types`, `go/ssa`, and Go callgraph packages for validation, not as the long-term runtime engine.

Reason:

- User wants full native implementation.
- polint needs one unified fact/provenance/cache model.
- Go official tools remain the best compatibility oracle.

## D7: Sparse Flow-Sensitive Analysis Comes Later

Decision: design for MemorySSA/SVFG-style sparse refinement, but do not build it first.

Reason:

- Dense flow-sensitive points-to is too expensive.
- LLVM/SVF show sparse is the scalable route.
- The first system needs places, local flow, summaries, and flow-insensitive points-to first.
