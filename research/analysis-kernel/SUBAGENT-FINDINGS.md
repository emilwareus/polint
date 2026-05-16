# Subagent Findings

This research used five parallel research agents plus local synthesis.

## 1. Incremental Query Engines

Focus:

- Salsa;
- rust-analyzer;
- TypeScript incremental builder;
- Pyre dependency-tracked memory;
- gopls analyzer cache;
- FlowLog/CodeQL incremental angles.

Findings:

- Salsa's red-green algorithm, durability, backdating, and snapshots are the right concepts, but adopting Salsa as the first kernel storage engine would be premature.
- rust-analyzer shows the importance of semantic invalidation barriers. Not every source edit should invalidate global facts.
- TypeScript shows the value of public shape/signature digests.
- Pyre shows that absence/presence dependencies matter, not only value dependencies.
- gopls shows a practical provider recipe-key model for package/analyzer caches.

Incorporated into:

- `RECOMMENDED_IMPLEMENTATION.md` phases 3 and 6;
- `algorithms/cache-invalidation.md`;
- `RESEARCH-ANALYSIS.md` sections on Salsa, TypeScript, gopls, and Pyre.

## 2. Datalog And Relation Engines

Focus:

- Souffle;
- Doop;
- CodeQL;
- FlowLog;
- IncIDFA.

Findings:

- Recursive facts need relation/fixpoint thinking: SCC scheduling, delta sets, relation-local indexes, and monotone merge.
- Souffle/Doop are excellent internal models but too heavy as the first public rule API.
- CodeQL's product shape is more important than its full query language: extracted facts, typed object views, least fixed point recursion, path graphs, model provenance.
- FlowLog and IncIDFA are future incremental directions, not first-slice requirements.

Incorporated into:

- `FINAL-REPORT.md` relation/fixpoint recommendation;
- `oss/implementation-comparison.md`;
- `algorithms/provider-scheduling.md`.

## 3. Graph/Product Analysis Kernels

Focus:

- Joern overlays;
- WALA analysis products;
- OpenRewrite markers/data tables;
- CodeQL path evidence.

Findings:

- Joern's overlays validate the layer concept, but polint should plan dependencies rather than skip missing layers.
- WALA is the best reference for explicit typed analysis products: IR, CFG, call graph, pointer analysis, IFDS, slicing.
- OpenRewrite shows useful evidence-table and controlled multi-pass patterns.
- CodeQL path graphs are the right inspiration for data-flow/call/effect evidence.

Incorporated into:

- `STANDARD.md` layer model;
- `VALIDATION.md`;
- `RECOMMENDED_IMPLEMENTATION.md` relation/fixpoint and API discipline sections.

## 4. Provenance, Precision, Validation, Merge Safety

Focus:

- CodeQL model provenance;
- Pysa model validation;
- Semgrep taint traces;
- Joern custom semantics;
- SARIF evidence export.

Findings:

- Precision and confidence must be separate.
- Extension facts must be validated and promoted; they should not write directly into core fact storage.
- Default merge should be union. Exact conflicts should fail. Suppressions and neutral facts are high risk.
- SARIF is a good export target, not an internal schema.

Incorporated into:

- `STANDARD.md` fact envelope, precision, confidence, provenance, validation, and merge sections;
- `VALIDATION.md`;
- `algorithms/extension-merge-validation.md`.

## 5. Current polint Integration

Focus:

- `runner::analyze_and_run`;
- `AnalysisDb`;
- `AnalysisPlan`;
- cache keys;
- Go/TS adapters;
- module graph, symbol graph, metrics;
- SDK fact views and capability support.

Findings:

- The correct first integration point is inside the local rules-host runner pipeline.
- `AnalysisDb` can remain initially; add a kernel orchestration boundary and sidecar metadata instead of rewriting storage first.
- Current parser cache keys are safe but over-invalidate because they include rule and plan digests.
- Capability support should become provider output and validation output, not only a precomputed plan field.

Incorporated into:

- `RECOMMENDED_IMPLEMENTATION.md` phase plan;
- `FINAL-REPORT.md` current-polint analysis;
- `REPO-INDEX.md` current integration points.

