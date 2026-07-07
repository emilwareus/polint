# Open Questions

Decision log for the static-analysis-2.0 track. One entry per decision.
Status: `open` | `decided(<summary>)` | `deferred`.

Validation pass: 2026-07-07. Rechecked the recommendations against the current
checkout and current external primary sources:

- Jelly documents NodeProf-backed dynamic call graph construction and
  comparison (https://github.com/cs-au-dk/jelly), and approximate
  interpretation remains the right reference point for the JS/TS real-app
  oracle path (https://cs.au.dk/~amoeller/papers/approx/).
- TypeScript 7.0 RC still does not expose a stable programmatic API until
  TypeScript 7.1 or later, so the production sidecar cannot depend on tsgo APIs
  yet (https://devblogs.microsoft.com/typescript/announcing-typescript-7-0-rc/).
- CodeQL's data-extension model rows are the strongest existing public precedent
  for source/sink/summary/type/barrier summary data
  (https://codeql.github.com/docs/codeql-language-guides/customizing-library-models-for-javascript/).
- Local semantic-store research supersedes the earlier redb-first store
  recommendation: SQLite/rusqlite is now the primary mutable local store because
  graph queries, filters, migrations, and search manifests are first-class
  product requirements; redb remains the pure-Rust fallback
  (../local-semantic-store/FINAL-REPORT.md).
- Local correction: current `analysis/data_flow/direct_calls.rs` already
  consumes `db.refined_call_edges()`, so the 08 data-flow decision is to keep
  that behavior under `dataflow`, not introduce a new capability.

## 01 — Benchmarking & measurement

- **Q1** [decided(real-app JS/TS corpus)] Use the Jelly PLDI'24 artifact as the
  first corpus source. Pick 8 projects from the 36 with dynamic call graphs,
  stratified by size, framework style, module system, and dependency volume.
  Trust artifact traces for the baseline; rerun NodeProf for 2-3 canaries per
  release to catch drift in our adapter/tooling.
- **Q2** [decided(Go oracle split)] Use `golang.org/x/tools` RTA as a recall and
  consistency reference, not a precision oracle. Add curated required-edge sets
  for realistic Go repos/frameworks such as `gin`, `prometheus`, `terraform`,
  and one large monorepo like `kubernetes`.
- **Q3** [decided(dual lanes, no blend)] Report oracle modes separately:
  `recall@dynamic`, `precision@required`, `unclassified_extra_edges`, runtime,
  peak RSS, and budget events. Do not publish a blended F1 across dynamic and
  required-edge oracles.
- **Q4** [decided(external RSS sampler)] Use an external process-tree peak-RSS
  sampler as the primary measurement. Keep `getrusage` as secondary metadata.
  CI gates should compare medians over repeated runs with tolerance bands, not
  hard absolute RSS values.
- **Q5** [decided(first-class budget facts)] Budget exhaustion becomes both
  telemetry and first-class degraded-analysis facts. Rules/agents must be able
  to ask whether a file, call site, flow, or provider result was truncated.

## 02 — Memory & representation

- **Q6** [decided(lazy snippets + stable spans)] Keep line-offset tables, source
  spans, and file digests; lazy re-read source only when rendering diagnostics.
  If the file digest no longer matches, omit the snippet rather than retaining
  source in memory.
- **Q7** [decided(hybrid set container)] Start with a hybrid points-to set
  representation: sorted inline small vectors for small sets, promoted to
  hash-consed roaring or fixed bitsets for large dense sets. The exact promotion
  threshold must be chosen from real-corpus token-set distributions.
- **Q8** [decided(indirection primary, token cap safety)] Keep
  `max_tokens_per_cell` as a safety fuse, but make `max_indirection_depth` the
  primary scale knob. Provisional defaults: review=3, check=4, CI=6,
  benchmark/high-precision=unbounded or high. Always surface budget loss.
- **Q9** [decided(drop MIR/CFG post-summary)] After summaries land, drop function
  MIR/CFG by default once summaries and required facts are extracted. Re-lower on
  demand for evidence, changed functions, and explicit debug/inspection.

## 03 — Summary store

- **Q10** [decided(v1 includes value-flow and TITO slots)] v1 summary schema
  includes call graph stitching, callback invocation behavior, value-flow
  (param-to-return and field escape), and TITO taint-transfer slots. Defer
  broad source/sink catalogs, not the taint-transfer shape.
- **Q11** [decided(hybrid granularity)] Use dependency summaries at the
  export/API boundary with package-level manifests; use per-function/per-SCC
  summaries for app code.
- **Q12** [decided(scoped Merkle key)] App-code summary key =
  schema/version digest + frontend/toolchain digest + body hash + scoped config
  digest + callee summary digests + extension/model digest. Config changes must
  invalidate only affected domains, not the whole repository.
- **Q13** [decided(SQLite/rusqlite semantic store)] Use SQLite through
  `rusqlite` with bundled SQLite as the primary mutable local semantic store for
  facts, summaries, graph indexes, provenance/status metadata, and search
  manifests. The queryable local CLI requirement changes this from a pure
  summary-cache problem into an indexed relational/graph-adjacent store problem.
  Keep `redb` as the best pure-Rust fallback and possible content-addressed blob
  cache. Avoid RocksDB/Kuzu/DuckDB/sled as v1 defaults. Keep flat mmap/rkyv
  shards for future immutable package-summary export artifacts, not the primary
  mutable store. See `research/local-semantic-store/`.
- **Q14** [deferred(local+CI cache first, registry-enabled seams)] Do not build
  the remote registry in the first implementation: no server, publish/fetch
  protocol, public package corpus, or registry operations work now. Prove the
  embedded local store and CI cache restore first. The local summary format must
  still preserve registry-ready seams: content-addressed package summaries,
  package/version identity, schema versions, provenance, validation metadata,
  trust hooks, and recompute-and-diff support. If a registry is later built,
  start private/team-first with signed digest entries and spot recomputation.
- **Q15** [decided(precision floor via query options)] Summary-derived facts use
  existing precision/confidence tiers. Default summary facts are `Conservative`
  or `SetupAware`; AI/approximate summaries are `Heuristic`; unknown-top
  summaries remain `Unknown`. Rule-facing query views keep `minimum_precision`
  and `minimum_confidence`.
- **Q16** [decided(refinement overlays run-local)] Persist canonical local/SCC
  summaries. Keep whole-program refinement overlays run-local unless their full
  closure, config, and inputs are represented in the key and validated against
  from-scratch recomputation.

## 04 — Incrementality

- **Q17** [decided(Salsa-shaped first)] Build a Salsa-shaped internal query
  boundary first. Adopt the Salsa crate only after a narrow prototype proves
  memory use, deterministic outputs, cancellation behavior, and parallel
  behavior are acceptable.
- **Q18** [decided(add determinism fixtures)] Existing determinism gates are not
  sufficient. Add cold/warm/partial runs, edit-order permutations, parallelism
  permutations, and process-restart fixtures.
- **Q19** [decided(rule-pack hash boundary)] Rule-pack edits invalidate rule
  execution by rule-pack hash. Analysis layers survive unless requested
  capabilities, analysis-affecting settings, rule options, or extension/model
  facts changed.

## 05 — Type-directed callgraph

- **Q20** [decided(Node sidecar now, tsgo later)] Prototype and ship the first
  TS type sidecar with Node + the TypeScript compiler API. Evaluate tsgo as an
  implementation target only after TypeScript exposes a stable programmatic API
  in 7.1 or later.
- **Q21** [decided(batched per-project dump)] Use batched per-tsconfig/per-file
  resolution dumps keyed by content and compiler options. Avoid per-call-site RPC
  chatter and avoid one giant whole-project type table.
- **Q22** [decided(any-density gates)] Never treat `any` as a high-confidence
  typed edge. Mark a file/module degraded around 25% unknown/`any` receivers; at
  roughly 50% defer most typed-tier receiver edges to field/heap tiers while still
  using exact non-`any` sites.
- **Q23** [decided(module_graph owns topology)] Reuse `module_graph` to discover
  workspace units, project references, package-manager topology, and path-alias
  scope. The sidecar consumes scoped project units and lets TypeScript resolve
  within each unit.
- **Q24** [decided(minimal ACG experiment)] Build a minimal ACG-style field-based
  tier only after the type tier. Keep it only if it shrinks heap residue or adds
  at least 2pp real-app recall without unacceptable precision/memory cost.

## 06 — Selective precision & demand

- **Q25** [decided(fan-in heuristic first)] First JS context-sensitivity
  heuristic: give 1-call-site sensitivity to the top 5% of functions by
  function-valued-parameter fan-in, requiring at least 3 distinct callers and a
  cap of 15% heap-constraint growth. Tune only against the real-app FP buckets.
- **Q26** [decided(typed query views)] Keep the v1.4 typed query-view style.
  Demand execution goes behind `Calls<'_>`, `ControlFlow<'_>`, and
  `DataFlow<'_>`; do not add `ctx.flows(...)` or expose raw graph traversal.
- **Q27** [decided(shadow then retire)] Retire recognizers family-by-family only
  after a typed/field/heap tier shadows the same behavior with equal-or-better
  benchmark and fixture results. Keep direct lexical binding; label legacy
  recognizer facts heuristic until deletion.

## 07 — ML integration

- **Q28** [decided(reuse-first type model)] Reuse CodeTIDAL5/JoernTI-style
  weights first behind propose-then-verify; fine-tune second if it moves the
  project-specific FN buckets. Models are installed explicitly (`polint ml
  install` or equivalent), SHA-pinned, and never downloaded during `polint check`.
- **Q29** [decided(opt-in locked ML facts)] ML inference is opt-in per repo.
  Transformer-style inference materializes reviewable, committed ML-facts
  lockfiles with model/runtime digests; live inference is excluded from
  byte-stable `check` guarantees. A deterministic tree ranker may run live.
- **Q30** [decided(real traces before ranker)] Existing micro fixtures are enough
  for feature sanity checks only. Train rankers only after real-app dynamic
  traces exist; hold out whole packages to reduce test-coverage bias.
- **Q31** [decided(CodeQL-style summary rows)] Use CodeQL Models-as-Data-style
  rows for LLM summaries: source, sink, summary, type, barrier, access path, and
  provenance. Require structured generation, majority vote, `.d.ts` conformance,
  and optional dynamic promotion. AI rows are recall-only and heuristic unless
  manually or dynamically confirmed.
- **Q32** [decided(agent-side triage)] Triage belongs on the consuming agent side,
  not in `polint check`. polint emits evidence bundles, provenance chains,
  confidence, honesty tiers, counter-evidence hooks, and stable finding IDs.

## 08 — Data-flow & taint

- **Q33** [decided(use existing dataflow capability)] Do not add a
  `dataflow-refined` capability. Keep refined-call-powered behavior under the
  existing `dataflow` capability with precision/provenance labels. Local code
  already consumes `refined_call_edges()`.
- **Q34** [decided(precision-first default)] Review rules default to
  under-approximate, precision-first interprocedural flow. Rules can opt into
  recall-first may-flow behavior by lowering precision/confidence thresholds.
- **Q35** [decided(summary segment evidence)] Evidence crossing summarized
  boundaries cites a compact summary segment: package/version or app digest,
  callable, transfer, summary digest, precision tier, and provenance. Expansion
  is on demand, not default.

## Strategy / sequencing

- **Q36** [decided(substrate before language #3)] Decide language #3 after the
  summary store and type-tier architecture are proven. Default next language is
  Python; choose Java earlier only if enterprise demand dominates. Rust comes
  later.
- **Q37** [decided(policy views before raw facts)] Do not stabilize raw
  CG/CFG/DF SDK shapes now. Promote policy query views first, and stabilize raw
  graph/fact views only after tier labels, summary evidence, and real-app gates
  settle.
- **Q38** [decided(Jelly micro as regression net)] Freeze Jelly micro as a
  regression net at the current plateau. The headline metric becomes real-app
  F1/recall/precision curves plus RSS/time/budget-vs-size.
- **Q39** [decided(sandboxed opt-in)] Approximate interpretation is not a default
  path. It may be offered later as an explicit, sandboxed opt-in: no network,
  read-only inputs, resource limits, and heuristic/approx provenance.
- **Q40** [decided(byte-stable scope)] Promise byte-stable output for identical
  source, config, toolchain, lockfiles, rule-pack digests, extension/model
  digests, and ML-facts lockfiles across cold/warm/parallel runs. Exclude live ML
  inference and unsandboxed dynamic execution from the deterministic guarantee.
