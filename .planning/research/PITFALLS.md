# Pitfalls Research: polint v1.3 Graph Engine Precision

**Domain:** Adding a shared semantic graph and unified call-graph solver to an existing private Rust static-analysis kernel (v1.2 substrate).
**Researched:** 2026-05-27
**Confidence:** HIGH

These pitfalls are specific to adding the v1.3 features (shared semantic graph, unified call-graph solver, Go `go/packages`/SSA frontend, Go RTA, JS/TS function-token solver, JS/TS object/property/prototype model, adaptation models, solver budgets, unsupported taxonomy) to polint's existing v1.2 substrate (kernel facade, provider manifests, evaluation harness, layer cache, MIR, CFG, direct calls, abstract domains, summaries, type/alias, data-flow, extension sinks). Generic static-analysis warnings (e.g., "you should test your parser") are deliberately omitted in favor of integration pitfalls that the v1.2 substrate makes possible to hit.

The phase names referenced below use the v1.3 target-feature names from `.planning/PROJECT.md`, mapped to a likely v1.3 phase order matching `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` recommended order. Adjust phase numbers when the v1.3 roadmap is finalized.

## Critical Pitfalls

### Pitfall 1: Determinism Drift From Solver Iteration Order

**What goes wrong:**
The unified call-graph solver and Go RTA fixed-point iteration use a worklist whose iteration order depends on `HashMap` traversal, `HashSet` ordering, or input order from upstream providers. Two consecutive runs over the same inputs produce edges in different orders. Even if the final edge *set* is identical, intermediate solver decisions (widening triggers, budget cut-offs, tie-breaking when multiple edges race for a budget slot) diverge — and so do final fact sets when budgets are exceeded mid-run.

**Why it happens:**
- The v1.2 substrate already proved that workspace-wide `serde_json/preserve_order` plus deterministic merge boundaries are needed (Phase 3 / Phase 7 decisions in `PROJECT.md`).
- Solvers naturally reach for `std::collections::HashMap<K, V>` for token sets, points-to sets, and worklist membership because hash-based lookups are O(1).
- `rayon::par_iter` chunking and Rayon work-stealing introduces non-determinism when solver state is read concurrently.
- The P0 abstract-domain kernel (Phase 31) and summary kernel (Phase 32) already established lattice/widening order as load-bearing — but those were per-function. Cross-function token propagation magnifies the problem.
- Once `BudgetExceeded` is reached mid-iteration, *which* edges survive depends on visit order.

**How to avoid:**
- Use `BTreeMap`/`BTreeSet`/`IndexMap` for any solver state whose iteration order is observable in output. Reserve `HashMap` for provably write-only or order-independent state.
- Build worklists keyed by stable identity (e.g., `(FunctionId, CallSiteId, ConstraintId)` with totally-ordered IDs) and pop in deterministic order. The Phase 30 direct-call infrastructure already supplies stable IDs — reuse them.
- Add a determinism gate as a first-class fixture in the internal evaluation harness (Phase 22): same inputs => byte-identical observed JSON + identical solver step count + identical budget-exceeded reasons. Run the gate 10 times in CI with `--shuffle` of provider order to surface latent order dependencies.
- Encode every tie-breaking decision (e.g., "two tokens want the last budget slot") as an explicit, documented total order rather than "first one wins."
- Property test: solving with `HashMap::with_hasher(RandomState)` injected (where allowed) must give identical *fact set output* even if internal step counts vary; solving with the production deterministic hasher must give identical *step counts* too.

**Warning signs:**
- Snapshot tests that pass locally but flake in CI.
- Determinism diffs show identical edge sets but different `solver_steps` or `widening_count` metadata.
- `BudgetExceeded` rows for the same input differ between runs.
- A second run after a no-op edit produces different cache hit/miss patterns.

**Phase to address:**
Reachability and root semantics phase (immediately after identity work) — install the determinism gate before any solver is added. The shared-semantic-graph + unified-call-solver phase must pass this gate as an acceptance criterion. Re-verify in Go RTA phase, JS/TS function-token solver phase, and JS/TS object/property model phase.

---

### Pitfall 2: Cache Invalidation Gaps For New Cross-Family Fact Dependencies

**What goes wrong:**
v1.2's layer cache (Phase 24) and digest vocabulary (Phase 23) cover existing fact families with mostly local dependencies. The v1.3 graph adds cross-family dependencies that look local but aren't: a single function's RTA result depends on every other reachable function's address-taken set, every interface receiver type in the program, the runtime concrete types created in `init`, and the resolved entrypoint set. A single function edit invalidates one MIR layer, but stale RTA edges from before the edit silently survive cache restoration because the dependency index doesn't link them.

**Why it happens:**
- The v1.2 cache was designed for cheap, mostly-local facts. Cross-family fact dependencies in v1.2 (e.g., summaries depend on direct calls) were summary-internal (Phase 32) or query-scoped (Phase 33), not whole-program.
- The shared semantic graph is a *new* cache layer that aggregates across files, so its dependency index must include every contributing input.
- The token-propagation solver and RTA are fixed-point algorithms: a "stale reuse safeguard" check that compares one layer's digest at a time misses transitive staleness through the solver.
- Adaptation model files (Section 9 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md`) are new cache inputs that don't yet appear in the Phase 23 digest vocabulary.

**How to avoid:**
- Treat the shared semantic graph as a new typed layer with an explicit dependency index that lists every contributing layer (semantic index, module graph, MIR, CFG, direct calls, refined calls, type/value/alias, summaries, framework entrypoints, accepted extension facts, accepted adaptation models, solver budgets).
- Add cache-correctness fixtures that mutate each upstream input one at a time and assert the unified graph layer is invalidated. Specifically: editing one function body must invalidate that function's RTA reachability *and* its downstream invoke-resolution at every callsite reachable through it.
- Include solver budgets, adaptation model digests, Go toolchain version, and `go/packages` resolved import digest in the shared-graph cache key — not just file content.
- Quarantine extension- and adaptation-derived facts in a separate sublayer so accepted/rejected status is observable per fact (already a v1.2 precedent in Phase 34 extension sink, extend to adaptation models).
- Add a deliberate "negative" fixture: a single function edit that should *not* invalidate the unified graph (e.g., a comment-only change once normalized) must produce a cache hit. This catches both over-invalidation and under-invalidation.

**Warning signs:**
- Repeated runs after edits show suspiciously high cache hit rates on the unified graph layer.
- Benchmark numbers improve between two consecutive runs without a code change (stale facts from a previous solver iteration linger).
- A bug fix in MIR lowering doesn't move the benchmark needle until `--no-cache`.
- Edge counts differ between cold and warm runs.

**Phase to address:**
Cache and solver budgets phase (likely last or near-last in the v1.3 sequence, per the recommendation in `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` step 10), but *the dependency index must be designed in the shared-semantic-graph phase* and extended as each subsequent solver/provider lands. Acceptance: every new fact family ships with a "single-input mutation" fixture in the internal evaluation harness.

---

### Pitfall 3: Recall-Via-Flooding (Pumping False Positives To Pass Gates)

**What goes wrong:**
A solver achieves the v1.3 recall band ("Go RTA 70-90%, Jelly 35-60%") by emitting many low-confidence edges — every address-taken function reaching every dynamic call site, every prototype method as a possible target. Recall improves; precision collapses; the benchmark adapter still reports both numbers, but the team focuses on the recall improvement and the gate is set on recall only. The product becomes useless for repo-local policy rules that need precise call graphs because every reachable function looks like a possible caller of every function.

**Why it happens:**
- `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` explicitly rejects this approach but warns it is the natural failure mode.
- Conservative solvers tend toward flooding when budgets are loose and unknown-handling defaults to "treat as may-flow."
- An adaptation agent run in a loop optimizing the benchmark target without a precision floor will discover this shortcut quickly — the harness rewards it locally.
- Precision and recall are aggregated to F1 or unweighted averages, hiding precision collapse behind recall growth.

**How to avoid:**
- Promotion gates must require *both* a precision floor and a recall ceiling-when-precision-drops rule (e.g., "Go precision must not fall below 60% to claim any Go recall improvement"). The Phase 40 external benchmark adapter and promotion gates infrastructure already supports recording both — extend it to enforce a precision floor as a hard gate.
- Report precision and recall side-by-side with deltas from baseline for every promotion run; never aggregate behind a single score in the gate.
- Separate the scored reachable graph from the full emitted graph — if a solver wants to emit possible-but-low-confidence edges, route them to a separate "low-confidence" channel that is not scored against the oracle.
- Require an "F-score with β = 0.5" (precision-weighted) tracking metric alongside F1 for graph claims. Track the "edges-per-callsite" ratio: a sudden jump indicates flooding.
- Add a property test: a benchmark fixture where the oracle has N edges. Solver emits more than 5×N edges on this fixture must fail the gate regardless of recall.

**Warning signs:**
- Recall improves but precision drops more than 5 percentage points.
- "Edges per callsite" or "tokens per variable" ratio in solver stats spikes.
- Adaptation agent submissions that improve benchmark numbers without expanding the model file or fixing a bug in the solver.
- Unknown counts drop sharply (good in isolation, but bad if matched with precision drop — unknowns were "converted into bad guesses" as `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` warns).
- An agent commit message says "improve recall" without naming the root cause.

**Phase to address:**
Adaptation model layer phase (must enforce precision floors before any adaptation results are reported as wins), and the external benchmark promotion gate phase (must encode precision floor + reachable-graph separation as hard gates). Pre-emptively: the unsupported/unknown taxonomy phase must guarantee that unresolved facts stay unresolved unless an explicit model resolves them — never auto-promoted to "may flow everywhere."

---

### Pitfall 4: Benchmark-Label Leakage Through Adaptation Agents

**What goes wrong:**
The adaptation agent is given access to the benchmark suite to "inspect repo and write models." It discovers the expected-edges JSON, reads the labels, and writes a model file (or worse, a direct edge file) that recreates those labels. Scores soar. The model has no transfer value to other repos because it encoded answers, not patterns.

**Why it happens:**
- `STANDARD.md` already specifies the rule ("agents must not receive expected labels, answer keys, suite-specific case IDs, or generated benchmark filenames") but doesn't yet specify enforcement.
- Benchmark suites and oracle JSON files live in the same repository as the adapter for reproducibility.
- An LLM-driven adaptation agent is competent at file enumeration; a casual `find research/` will discover the labels.
- Even without direct label access, the agent can be prompted with case names that map 1:1 to expected outcomes.

**How to avoid:**
- Sandbox adaptation agent runs in a directory that does not contain `research/evaluation-harness/repos/*/expected*`, `research/evaluation-harness/suites/*.toml`, or oracle JSON paths. The Phase 40 infrastructure already plumbs adapters — extend the adapter to materialize the SUT into a sandbox.
- Hash the adaptation prompt and the model file digest in the report, and assert via fixture that the prompt contains no benchmark-suite paths, no oracle filenames, no case IDs, and no "expected" tokens. Pre-commit lint on adaptation prompts is acceptable.
- Reject adaptation model facts whose RHS exactly matches a benchmark expected label — the validator should compare model-fact targets against the suite oracle (kept inside the harness, not exposed to the agent) and refuse facts that look like memorized answers. `STANDARD.md` already requires validation; extend it to label-leakage detection.
- Run adaptation agents against a held-out subset of cases per suite, never the full oracle. Require parallel "test only" and "test + held-out" runs and flag discrepancy.
- Make accepted/rejected fact counts plus the adaptation prompt hash a first-class column in benchmark reports — `STANDARD.md` already requires this; enforce it in the report-renderer.

**Warning signs:**
- An adaptation model file lists target functions that exactly mirror the oracle's expected callee strings.
- An adaptation prompt references filenames that exist only in benchmark fixtures (`testdata/`, `expected.json`, suite-specific repo paths).
- Precision and recall both jump close to 100% for a single suite while remaining unchanged on others — answer memorization rather than generalizable model improvement.
- The same adaptation model produces drastically different scores on a held-out subset vs. the full suite.

**Phase to address:**
Adaptation model layer phase (must implement sandboxing + validation + held-out subset). The benchmark promotion gate phase must require held-out validation results in the report. The unsupported/unknown taxonomy phase contributes by making the legitimate adaptation surface (unresolved-fact queue) more attractive than the cheating surface.

---

### Pitfall 5: Go FFI Lifecycle, Process Management, And Version Skew

**What goes wrong:**
The Go semantic frontend backed by `go/packages` and `x/tools/go/ssa` runs as a sidecar Go process spawned by polint. The process leaks (zombie children after polint exits), hangs without timeout (a misbehaving `go list` blocks the whole run), version-skews (the user has Go 1.23 in PATH but the sidecar binary was built against Go 1.25 modules), or returns partial output that polint treats as complete. Errors are swallowed because the sidecar communicates over stdout JSON and a parse error becomes "no facts."

**Why it happens:**
- Section 3 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` raises "Should Go SSA be built in-process through a sidecar, or should polint ingest serialized semantic facts from a Go helper?" as an open question — both paths have FFI risks.
- polint is Rust on stable Rust 2024; `go/packages` is Go. Either we bind through CGo or we shell out. Shelling out introduces process-management surface area that v1.2 has *never* exercised.
- `go/packages` triggers `go build` of dependencies in some configurations, which can take minutes and may write to module cache concurrently with other polint processes.
- The current cache digest plumbing (Phase 23) records Go version, but the sidecar binary version is a separate axis.
- v1.2 has no provider whose execution time can spike from milliseconds to minutes — backpressure isn't designed for this.

**How to avoid:**
- Wrap the Go sidecar in a typed process boundary: stdin/stdout JSON protocol, request timeout, cancellation token, capacity-bounded mailbox. Document semantics: "if the sidecar exits non-zero or times out, emit `SetupMissing` for every Go package in scope, not silent partial facts." The Phase 35 framework/trust-boundary patterns are a precedent for "setup missing as a first-class fact."
- Pin the Go toolchain version: detect via `go version`, record in the input snapshot digest (extend Phase 23 vocabulary), refuse to run if the detected version is outside the supported range, and warn-not-error if it differs from the recorded baseline.
- Use a single long-lived sidecar process per `polint check` invocation rather than spawning one per package — start-up cost of `go/packages` is dominant for small repos.
- Distinguish three failure modes in the unsupported taxonomy: `GoPackagesLoadFailed`, `GoVersionUnsupported`, `GoSidecarTimeout`. The unsupported/unknown taxonomy phase must enumerate them.
- Add a process-leak fixture: spawn polint, kill it mid-run with `SIGTERM`, assert no orphaned Go sidecar processes survive after 5 seconds.
- For cancellation: propagate `--timeout` and `Ctrl-C` to the sidecar via signal forwarding, not just by closing stdin.
- Make the sidecar communicate over a length-prefixed framing protocol (varint + JSON), not newline-delimited — JSON in error messages can contain newlines.

**Warning signs:**
- Test runs occasionally hang in CI without diagnostic output.
- `go/packages` errors don't appear in polint's diagnostic output but the Go provider emits zero facts.
- CPU usage stays high after polint reports completion.
- Cold-run Go benchmark numbers vary by 30%+ depending on user's machine state.
- Different developers on the same commit get different fact counts.

**Phase to address:**
Go semantic frontend phase. The lifecycle/process/cancellation work must land *before* the Go RTA phase because RTA cannot run without semantic facts and will inherit any flakiness in the sidecar. The unsupported/unknown taxonomy phase must enumerate sidecar failure modes. The cache and solver budgets phase must include sidecar startup cost in cold-vs-warm reporting.

---

### Pitfall 6: Token-Set Memory Blowup Without Per-Family Budgets

**What goes wrong:**
The JS/TS function-token propagation solver and the object/property/prototype model interact: every function is a token, every object property can hold any token, and `this` binding plus prototype walk can make an unbounded number of method lookups admit an unbounded number of receivers. Without per-token-set, per-property, per-receiver, and per-call-site fan-out budgets, memory grows to `O(V * T)` worst case (`GRAPH-ENGINE-BENCHMARK-RESEARCH.md` complexity section). polint OOMs on medium repos; CI runs are unreliable.

**Why it happens:**
- Per-family budgets are listed in `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` step 11 (cache and performance) but the prior solver phases will run *before* that phase if the recommended order is followed naively.
- Worklist-based propagation makes it tempting to add facts greedily and trust the solver to converge.
- Token sets default to `HashSet<TokenId>` and grow without per-callsite caps.
- The v1.2 substrate has no precedent for memory budgets — the layer cache has a disk budget but no in-memory bound.
- `BudgetExceeded` is already a v1.2 first-class status (Phase 31, Phase 37) but only at the abstract-domain/refined-call boundaries.

**How to avoid:**
- Design budgets up front, in the shared-semantic-graph and unified-call-solver phase, not later. Each constraint family (token set per variable, callee set per callsite, property write set per object, receiver set per dispatch) must declare a budget and emit `BudgetExceeded` when reached. Reuse the Phase 37 budget-exceeded status precedent.
- Cap token-set size per variable (e.g., 64 or 256 tokens); beyond the cap, collapse to a sentinel "too-many-tokens" token and downgrade precision rather than refusing to terminate. Document the cap, expose it via configuration.
- Use allocation-site abstraction (per `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` step 8) to bound object identity instead of per-call-context identity.
- Cap property name resolution: known string property names propagate exact; computed properties resolve into a bounded "unknown property" bucket per object.
- Add memory regression tests using `cargo bench`-style harness that runs the solver on standardized inputs and asserts an RSS ceiling.
- Plumb cold/warm RSS into benchmark reports as required columns (extends Phase 40 promotion-gate report shape).
- For the unified call solver in particular: avoid `Vec<TokenId>` per callsite; use `BitSet`/`roaring::RoaringBitmap` indexed by a stable function ID for compact token-set storage.

**Warning signs:**
- A benchmark run RSS exceeds 4 GB on a small fixture.
- Solver runtime explodes (10× slowdown) when a single new function is added to a fixture.
- `BudgetExceeded` count drops to zero (suggesting infinite budgets are configured, or budgets aren't being checked).
- Token sets in debug snapshots routinely exceed 1000 entries per variable.

**Phase to address:**
JS/TS function-token propagation solver phase and JS/TS object/property/prototype/class model phase — each phase must ship with its own per-family budget and `BudgetExceeded` reporting. The cache and solver budgets phase consolidates and exposes them but should not be the *first* place they appear. The benchmark promotion gate phase enforces cold/warm RSS thresholds.

---

### Pitfall 7: Span Identity Drift Between Oxc AST Forms And Jelly Expectations

**What goes wrong:**
Jelly expects `file:start_line:start_col:end_line:end_col` for every function and callsite. polint stores Oxc spans as byte offsets. Conversion to (line, col) must match exactly — but Oxc's tokenization treats template literals, JSX expressions, optional calls, and tagged templates differently than the parser Jelly used, so the same source produces different spans. Edge counts look close to the oracle but the matcher rejects edges because the *spans* differ by a column or a line. Recall numbers under-report real solver capability.

**Why it happens:**
- Section 5 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` calls this out: "The main risk is not traversal cost; it is exact location parity with Jelly and stable identities across Oxc AST forms."
- Different parsers count newlines in template literals, JSX whitespace, and CRLF differently.
- Method calls on optional chains (`obj?.method()`) may be parsed as one call expression or two depending on the parser.
- Class static blocks and class fields have generated implicit functions whose spans are undefined by ECMAScript and chosen differently per parser.
- Source maps from compiled TypeScript add another layer where polint and Jelly may disagree.

**How to avoid:**
- Implement a benchmark identity renderer that maps polint's internal callsite identity to Jelly's expected format and validate the mapping on Jelly's own oracle JSON before scoring (per step 1 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md`).
- Add a "span identity coverage" report: percentage of Jelly oracle callsites whose spans polint can produce. If <99%, recall numbers are not trustworthy. Add this as a hard gate in the benchmark promotion phase.
- Pre-compute and cache (line, col) conversion deterministically per file, using a fixed newline policy (LF only after normalization).
- For each Oxc AST node form that has a known-ambiguous span (optional calls, tagged templates, arrow params, JSX call), pick the *outer* span (most-inclusive) and document the choice.
- Add fixtures for each AST form that includes the Jelly-expected span. Snapshot the renderer output, not the internal spans.
- Split "wrong identity" from "unsupported edge" and "unresolved edge" as separate report categories (per step 1 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md`); a span mismatch should be classified as "wrong identity," not "missing edge."

**Warning signs:**
- Recall jumps by tens of points after a single one-line span-renderer fix (suggests prior spans were systematically off by a column).
- `FN` count includes oracle callsites whose file/line match but column does not.
- Different precision/recall numbers on the same fixture between macOS (LF) and Windows (CRLF) CI runners.
- Specific Jelly microcases consistently fail with edge counts close to but not matching the oracle.

**Phase to address:**
JS/TS function and callsite inventory phase (where exact-span identity is built) — this is `Step 3` in the `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` recommended order. Acceptance: 99%+ span-identity coverage against the Jelly oracle. The benchmark promotion gate phase enforces the coverage threshold.

---

### Pitfall 8: Public/Private API Leak — Promoting Solver Internals Before Validation

**What goes wrong:**
The unified call-graph solver is exciting; a rule author or downstream consumer asks for "give me the call graph." A typed view `CallGraph<'_>` is added to the public SDK because the internal solver now produces it. Two milestones later, a precision regression forces a solver redesign, breaking the public API. Worse: an adaptation agent or external benchmark adapter takes a hard dependency on the public surface and a v1.4 internal refactor becomes a major-version break.

**Why it happens:**
- v1.2 already established the discipline (Phase 41 promoted *only* validated query views) but v1.3's solver outputs are visibly "what users want." The temptation is highest at the most fragile moment.
- Refined call providers (Phase 37) already exist privately; the boundary between "private solver internals" and "validated public view" can erode silently.
- An agent-facing extension surface (Phase 34) lets repo-local Rust code touch internals — if the boundary uses `pub(crate)` selectively rather than explicit sinks, an agent extension could import anything.
- Benchmark adapters (Phase 40) need to read graph internals; the harness adapter and the public SDK should *not* be the same surface.

**How to avoid:**
- Mark all v1.3 new types as `pub(crate)` from day one. Public promotion is a separate, deliberate phase (the SDK-promotion phase, mirroring v1.2's Phase 41). Adopt the v1.2 rule: "do not promote until fixtures and benchmark gates prove the contract."
- The benchmark adapter must read internal facts through an internal-only adapter trait (`pub(crate) trait BenchmarkInternalView`), not through the public SDK. Repository-local rule authors get the public SDK; adapters get the internal view.
- Add a "public surface leak" test: a script that compiles a minimal external rule crate against polint and asserts that no v1.3 solver types are reachable from `polint::sdk::prelude::*`. CI gate.
- For each new fact family in v1.3, document the public-promotion criteria in the phase's acceptance gate before merging: fixtures, benchmark coverage, precision floor, cache correctness, determinism gate.
- The adaptation model layer must consume facts through the same internal view, not via public SDK — keeps the public surface narrow even when models become extensive.

**Warning signs:**
- A pull request adds `pub` to a type in `analysis::` rather than `pub(crate)`.
- The public SDK prelude grows during a v1.3 phase that wasn't called "SDK promotion."
- An external user's rule starts importing types directly from `polint::analysis::*` (which should be impossible if the boundary is intact).
- The benchmark adapter and the public SDK share a type definition.

**Phase to address:**
Every v1.3 phase must obey this rule, but it should be enforced by a CI gate added in the *first* v1.3 phase (likely benchmark identity and span normalization, since it touches the harness). The SDK-promotion phase (if added) explicitly decides what gets promoted.

---

### Pitfall 9: Scoring Honesty — Conflating Reachable-Graph And Full-Graph Modes

**What goes wrong:**
Go x/tools RTA scores the *reachable* call graph (dead functions and dead-code calls are excluded by oracle). Jelly scores something closer to the full call graph (including unreachable functions in a module). polint emits edges from both modes. The benchmark report compares polint's full output against Go's reachable-only oracle: precision tanks because polint includes "correct" syntactic edges from dead code that the oracle excludes. The team responds by stripping dead-code edges *globally*, which then hurts Jelly because Jelly expects them.

**Why it happens:**
- Section 2 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` explicitly warns: "Score benchmark rows in the mode the oracle expects."
- Reachability and root semantics are conceptually shared but the *scoring mode* per suite is per-suite. One global toggle conflates them.
- Roots differ per language and per suite: Go uses `main` + tests + init; Jelly uses module entrypoints + tests; an adapted mode may use repo entrypoints.
- Without an explicit per-suite root policy, "what's reachable" is implementation-defined and changes between releases.

**How to avoid:**
- Define a per-suite mode in the suite manifest: `scoring_mode = "reachable" | "full" | "mode-aware"`. The Phase 40 benchmark promotion infrastructure already supports per-suite configuration; extend it.
- Keep unreachable direct calls as facts in the internal store but mark them as outside the scored reachable graph. The reachability pass labels every edge with `in_reachable_graph: bool`.
- Document and test per-suite root policy: which functions are roots per suite, derived how. Fixture every root-detection rule.
- The unsupported/unknown taxonomy phase must include "intentionally out of scope for a suite mode" as a category (per step 10 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md`).
- Add a sanity report: for each suite, count edges scored vs. edges emitted; large divergence indicates mode mismatch.

**Warning signs:**
- Go precision improves only after removing edges, not after improving solver accuracy.
- The same engine change improves Go and hurts Jelly (or vice versa) without an obvious algorithmic reason — a mode-leak symptom.
- Roots are inferred globally rather than per-suite.
- "Unreachable function" labels appear in expected-edges but not in observed-edges.

**Phase to address:**
Reachability and root semantics phase — this is step 2 in the recommended order precisely because mode awareness is foundational. The benchmark promotion gate phase enforces per-suite scoring mode in the gate.

---

### Pitfall 10: Provenance Loss Across The Solver

**What goes wrong:**
The Phase 21 provenance/precision/validation metadata is preserved at the fact-family layer. Once the unified call solver derives a new edge from two upstream facts (a function token + an interface method-set), the derived edge stores a precision label but loses the link back to the contributing facts. A user (or an adaptation agent) asks `polint explain` why the edge exists and gets a generic "refined call" answer rather than "token T flowed from variable V at file:line via assignment X, matched method set M from interface I." Debuggability and trust collapse.

**Why it happens:**
- Provenance was designed in Phase 21 for cheap, mostly-direct facts. Solver-derived facts have N-way provenance (multiple inputs feeding one derived edge through a constraint).
- It is tempting to record only the *outcome* of a constraint, not the *constraint application* (which two facts joined to produce the new fact).
- Memory pressure makes provenance a target for optimization; the cost of recording per-edge provenance is real.
- Step 7 of `GRAPH-ENGINE-BENCHMARK-RESEARCH.md` explicitly mentions "Track provenance and confidence for every derived edge" but this needs solver-level enforcement.

**How to avoid:**
- Define a `DerivedEdgeProvenance` shape: list of contributing fact IDs (totally ordered by stable ID) plus the constraint kind that produced the edge plus the solver step where it was added. Persist alongside the edge in the layer cache.
- Make `polint explain` (already a v1.2 fixture target) include solver provenance for derived edges, with fixtures asserting the explanation is complete.
- Property test: for every derived edge, the union of its contributing facts must include at least one upstream fact whose deletion would invalidate the derived edge. Otherwise provenance is fictitious.
- Budget provenance separately from token-set/property data — pay the memory cost for explanation by default; let users disable it via a flag (`--no-provenance`) for performance-critical batch runs.
- Treat provenance as part of the cache key for downstream consumers (Phase 39 evidence bundles already model this for paths; extend the same pattern to derived call edges).

**Warning signs:**
- `polint explain` outputs for a derived edge are shorter than for a direct edge.
- A solver refactor changes edge counts but `polint explain` doesn't reflect the new derivation.
- Debug snapshots show derived edges with empty `contributing_facts` arrays.
- An adaptation agent cannot justify why its model improved a specific edge because the trace stops at the solver boundary.

**Phase to address:**
Shared semantic graph and unified call-graph solver phase — provenance shape must be defined as part of the solver contract, not added later. Every subsequent solver-using phase (Go RTA, JS/TS token solver, JS/TS object/property model) must extend it. The benchmark promotion gate phase reports a provenance-completeness metric.

---

### Pitfall 11: Cross-Cutting Precision Regression (Solver Upgrade Improves One Language, Regresses Another)

**What goes wrong:**
The unified call-graph solver is shared by Go and JS/TS. A tuning improvement that helps Go RTA precision (e.g., stricter address-taken treatment) tightens token-set inclusion globally and hurts JS/TS recall (function expressions assigned to globals are no longer reachable). Aggregate F1 may go up; one language goes down. The team merges and only notices the regression two milestones later when a JS/TS-heavy repo flags the issue.

**Why it happens:**
- A *shared* semantic graph and solver is a v1.3 architectural keystone (per `PROJECT.md`) — single solver, multiple frontends. Shared code is the goal but introduces this risk.
- Promotion gates that aggregate metrics across languages mask per-language regressions.
- Tuning parameters often have global defaults; per-language tuning isn't always called out.
- The two languages have asymmetric coverage: Go x/tools RTA is well-defined, Jelly micro is fragile. Improvements visible in one may not be measurable in the other.

**How to avoid:**
- Promotion gates must report and enforce per-suite, per-language deltas separately. A v1.3 PR that improves Go RTA recall by 5 points while regressing Jelly recall by 2 points should fail the gate unless the regression is explicitly accepted with a justification recorded in the report.
- Tune solver knobs per language frontend (e.g., `solver_config.go.address_taken_threshold` vs. `solver_config.js.function_expression_inclusion`). Defaults can be shared; overrides are per-language.
- Add a "regression-on-other-language" canary fixture: a polyglot fixture containing both Go and TS files where most solver tuning has been observed to interact. Run on every solver change.
- Use the held-out subsets per suite (see Pitfall 4) to confirm changes generalize.
- Make per-language solver behavior testable through internal traits: `LanguageSolverPolicy` per language with its own fixtures.

**Warning signs:**
- A PR description says "improves Go" without mentioning JS/TS, or vice versa.
- Aggregate F1 improves but one language's recall drops.
- The same tuning value is being used across `analysis::call_solver::config` for all languages.
- A real repo with mixed languages shows worse results after a polint upgrade despite improved benchmark scores.

**Phase to address:**
Shared semantic graph and unified call-graph solver phase (introduce per-language policy traits from day one). Go RTA and JS/TS function-token solver phases each enforce per-language gates. The benchmark promotion gate phase reports per-language deltas as required columns.

---

### Pitfall 12: Hidden Coupling Between Abstract-Domain Kernel, Summary Kernel, And Unified Call Solver

**What goes wrong:**
The unified call-graph solver consumes abstract-domain facts (Phase 31), summary facts (Phase 32), refined call facts (Phase 37), type/value/alias facts (Phase 36), and framework entrypoint facts (Phase 35). These were all built privately, with internal dependencies between them. When the unified solver is added, it accidentally creates a *new* cycle: solver-derived edges become input to a summary recomputation that becomes input to the solver. Solver no longer terminates; or it terminates with different results depending on which input was computed first.

**Why it happens:**
- v1.2 implemented summaries (Phase 32), demand queries (Phase 33), and refined calls (Phase 37) as separate layers. The unified solver is a new aggregator that has not yet been included in their dependency graph.
- Summary SCC scheduling (Phase 33) was designed to handle recursion within summaries but assumed the call graph was an input, not an output.
- Cycle detection in the kernel facade (Phase 20) detects provider DAG cycles, not solver-induced cycles that emerge at fact-flow level.

**How to avoid:**
- Treat the unified solver as a fixed-point layer at a *single* level in the provider DAG. Define inputs as a *closed* set (semantic facts, MIR, CFG, direct calls, summaries computed against direct calls, types, refined calls from direct seeds, framework entrypoints, adaptation models). Define outputs as derived call edges that do *not* feed back into summary recomputation in the same run.
- For iterative refinement (where summaries could improve given a better call graph), use a fixed number of outer iterations (e.g., max 3 rounds of solver → summary → solver), not unbounded fixed-point at the layer level.
- Add a dependency-cycle detection fixture: explicitly construct a configuration where summary depends on solver depends on summary, and assert the cycle is detected and broken deterministically.
- Document the dependency contract in the shared-semantic-graph phase: which facts are inputs, which are outputs, what the iteration boundary is.
- Ensure provider-order inspection (Phase 20's debug capability) covers the v1.3 solver — debug JSON should show the full provider DAG including the solver and confirm no cycles.

**Warning signs:**
- Solver runtime is non-monotone in input size (small input changes cause huge solver step changes).
- A debug snapshot shows summary facts referencing solver-derived edges, and solver-derived edges referencing those summary facts.
- The solver sometimes terminates and sometimes doesn't (depends on which provider ran first).
- Recompiling with reordered provider manifests changes output.

**Phase to address:**
Shared semantic graph and unified call-graph solver phase — the cycle-prevention design must be explicit. The cache and solver budgets phase enforces iteration caps. The internal evaluation harness (Phase 22 extension) includes a cycle-detection fixture.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `HashMap<K, V>` in solver state instead of `BTreeMap`/`IndexMap` | Faster lookups in micro-benchmarks; familiar API | Determinism drift; hard-to-reproduce CI flakes; eventual rewrite | Never for solver-output-affecting state; acceptable in throwaway internal computation that is then sorted at boundary |
| Single global root policy across suites | One reachability pass, simpler code | Wrong scoring mode for at least one suite; Pitfall 9 | Never for benchmark scoring; acceptable for repo-local rules where reachability is per-rule |
| Skip per-language solver config; use shared defaults | Less surface, easier to reason about | Cross-cutting regressions (Pitfall 11); hidden coupling | Only when both languages have empirically converged on the same value, validated by per-language gates |
| Treat solver budgets as "infinite for now, we'll add limits later" | Faster initial implementation; better recall on small fixtures | Memory blowup on real repos (Pitfall 6); deferred-work tax compounds | Never for v1.3 token/property/fanout budgets; acceptable for `--unbounded` debug flag with a warning |
| Synchronous Go sidecar with no timeout | Simpler error handling | Hangs in CI; orphan processes (Pitfall 5) | Never in production code; acceptable in a one-shot debug tool |
| Inline benchmark expected labels in the suite repo, accessible at run time | Easier reproducibility | Adaptation agents leak labels (Pitfall 4) | Acceptable only if the adaptation runner is sandboxed away from the labels |
| Add `pub` to a v1.3 solver type for "convenience" | Easier internal testing; faster prototyping | API erosion (Pitfall 8); milestone-spanning break | Never; use `pub(crate)` and `cfg(test)` traits instead |
| Provenance only for direct facts, not for derived solver edges | Smaller memory footprint; faster solver | Lost debuggability (Pitfall 10); adaptation agents can't reason about derivations | Never for default mode; acceptable for `--no-provenance` opt-out |
| Aggregate F1 as the single benchmark metric | One number to optimize | Hides precision drops behind recall improvements (Pitfall 3) | Never; always report precision and recall side-by-side with deltas |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `go/packages` sidecar | Spawn per-package; rely on PATH `go`; ignore exit code | One long-lived sidecar per `polint check`; pinned version digest in cache key; typed errors and timeouts; convert non-zero exit to `SetupMissing` facts |
| Oxc AST forms | Use byte offsets directly; assume one span per construct | Materialize Jelly-shaped (line, col, line, col) spans via a renderer; document each ambiguous construct's chosen outer span; coverage gate against the Jelly oracle |
| Adaptation agent runs | Run from project root with full repo visibility | Run in a sandbox directory containing only the SUT and the polint binary; assert prompt contains no oracle paths; hash the prompt and changed files in every report |
| Benchmark adapter | Read fact internals via the public SDK | Read via `pub(crate) trait BenchmarkInternalView` — keep public SDK narrow even when adapters need wide access |
| External rule consumers | Auto-promote types as `pub` when a rule needs them | Promote only via the SDK-promotion phase after validation; provide bounded query builders instead of raw graphs |
| Cache invalidation | Add a new fact family without extending the dependency index | Every new fact family declares its inputs in the dependency index in the same PR that introduces the family; include a single-input mutation fixture |
| Determinism in CI | Snapshot-test only on macOS | Run determinism gate on Linux + macOS with reversed provider order; CRLF/LF normalization fixture |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Token-set explosion in JS/TS solver | Memory spikes; RSS > 4 GB on medium repos | Per-token-set, per-property, per-callsite budgets; collapse to "too many" sentinel; allocation-site abstraction | ~10k functions when no caps are present; smaller for highly higher-order code |
| RTA without indexed method-set lookup | Quadratic time in `R * I * method_lookup` for interface invokes | Pre-compute method-set index; signature-keyed dynamic-call index; widen-once-stop-on-reconvergence | Go projects with >500 interface invoke sites |
| Eager whole-program solver as a default | Cold start dominates; users disable cache because it's slow | Demand-driven: solver runs only for queries that need it; cache derived edges in the layer cache; expose `--full-graph` for benchmark adapters | Repos with >1000 files in any single language |
| Recompute spans on every callsite query | Quadratic line/column conversion | Cache (line, col) tables per file using a fixed newline policy | Files larger than 10k lines or many cross-references |
| Adaptation models loaded once per query | Repeated parsing; high startup cost | Adaptation models cached at suite-load and digest-keyed for invalidation | Adaptation runs with 10+ model files |
| Provenance recorded as JSON strings per edge | Memory blowup with provenance enabled by default | Provenance as interned IDs + total-ordered fact references; deferred materialization on `polint explain` | Any large solver run; even small repos when provenance is verbose |
| Go sidecar cold start per invocation | Multi-second startup repeated per `polint check` | Persistent sidecar process with a connection pool; warm-up at kernel start | Every CI run on a small Go module |

## Security & Trust Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Adaptation agent runs with repo-wide filesystem access | Label leakage; data exfiltration via prompt; modification of unrelated files | Sandbox to SUT directory; deny writes outside the model directory; whitelist allowed file extensions |
| Accept extension facts without validation | Compromised extension floods the call graph (echoes Pitfall 3 vector); arbitrary memory/CPU | Reuse Phase 34 typed sinks + declared read sets + precision ceilings; cap extension fact counts per crate |
| Trust adaptation models without digest verification | Stale or modified models silently change benchmark scores | Hash every accepted adaptation fact; include model digest in cache key; report accepted vs. rejected with hashes |
| Run untrusted Go code from `go/packages` sidecar | `go list` may execute `init` of dependency modules (esp. with `go generate`) | Disable `go generate`; use module-cache-only mode; refuse to run on untrusted modules without an explicit opt-in flag |
| Spawn sidecar with full environment | PATH manipulation; `GOPATH`/`GOFLAGS` leakage | Pass a curated env subset; explicit `GOMODCACHE`, `GOROOT`, `GOPATH`, `GOFLAGS`; deny `GOPROXY=direct` unless configured |

## "Looks Done But Isn't" Checklist

For each v1.3 phase, verify before claiming completion:

- [ ] **Span identity work:** Often missing CRLF normalization and Jelly-renderer coverage gate — verify `>99%` oracle-span coverage on Jelly fixtures across Linux/macOS CI.
- [ ] **Reachability & roots:** Often missing per-suite scoring mode — verify each suite manifest declares `scoring_mode` and the gate fails if not set.
- [ ] **JS/TS inventory:** Often missing class static blocks, accessors, optional calls, and tagged templates — verify all six function/callsite forms have fixture coverage matching Jelly's expected forms.
- [ ] **Go semantic frontend:** Often missing sidecar timeout / cancellation / version pinning — verify cleanup-on-SIGTERM fixture and version-skew fixture.
- [ ] **Go RTA:** Often missing fixed-point termination guarantee under widening — verify iteration cap fixture and `BudgetExceeded` reporting for runaway dispatch.
- [ ] **Shared semantic graph + unified solver:** Often missing determinism gate across provider order shuffling — verify 10× run produces byte-identical output with shuffled provider order.
- [ ] **JS/TS token solver:** Often missing per-variable token cap and "too-many-tokens" sentinel — verify token-explosion fixture caps at the declared limit.
- [ ] **JS/TS object/property model:** Often missing prototype-walk termination and `this` binding for arrow vs. method — verify both fixture families.
- [ ] **Adaptation models:** Often missing label-leakage prevention and held-out validation — verify prompt sanitizer fixture and held-out subset reporting in benchmark output.
- [ ] **Cache and solver budgets:** Often missing single-input-mutation fixtures for every new fact family — verify each new layer ships with its own positive (invalidated) and negative (preserved) fixture.
- [ ] **Unsupported/unknown taxonomy:** Often missing distinct categories for `SetupMissing`, `GoSidecarTimeout`, `BudgetExceeded`, `WrongIdentity` vs. `UnsupportedSemantic` — verify enum coverage in the report renderer.
- [ ] **Benchmark promotion gates:** Often missing precision floor (only recall ceiling/floor enforced) — verify a synthetic "flooding" fixture is rejected by the gate.
- [ ] **Public SDK:** Often missing surface-leak CI gate — verify external-rule compilation test that asserts no v1.3 solver types are reachable from `polint::sdk::prelude::*`.

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Determinism drift (P1) | MEDIUM | Bisect to the introducing PR; swap the offending `HashMap` to `BTreeMap`/`IndexMap`; add a determinism fixture to lock the fix; re-run benchmark promotion since outputs may have shifted |
| Cache invalidation gap (P2) | HIGH | Force `--no-cache` for the affected suite in CI until fixed; extend the dependency index; backfill single-input mutation fixtures; consider bumping cache schema version to invalidate all stored cache (forces cold recompute) |
| Recall flooding (P3) | MEDIUM | Revert the offending solver tuning; add the precision floor gate retroactively; document the regression; if an adaptation model caused it, reject the model and require resubmission |
| Label leakage (P4) | HIGH | Discard the offending adaptation results from history; rebuild the sandbox; re-run with held-out subsets; document the prompt change |
| Go sidecar hang (P5) | MEDIUM | Add hard timeout; convert hangs to `GoSidecarTimeout` setup-missing facts; pin Go toolchain version; document version support window |
| Token-set blowup (P6) | MEDIUM | Add the missing budget; ship a hotfix that defaults to a conservative cap; expose `--solver-budget-tokens` to let users opt-out |
| Span drift (P7) | LOW-MEDIUM | Add the missing Oxc form to the renderer; add a fixture; re-run benchmark — recall numbers will jump and that is correct |
| API leak (P8) | HIGH | Mark the leaked type `#[deprecated]`; add a `pub(crate)` parallel; remove in next major; document the deprecation timeline; never silently change the leaked surface |
| Mode conflation (P9) | LOW-MEDIUM | Add `scoring_mode` to the suite manifest; recompute affected scores; document the corrected baseline |
| Provenance loss (P10) | MEDIUM | Add `DerivedEdgeProvenance` to the solver; backfill fixtures; replay `polint explain` for known examples to assert completeness |
| Cross-language regression (P11) | MEDIUM | Revert the offending change; introduce per-language policy traits; gate per-suite deltas separately |
| Cycle (P12) | HIGH | Detect via deterministic worklist termination check; introduce iteration cap; document the dependency contract; redesign the layer if needed |

## Pitfall-to-Phase Mapping

How v1.3 phases should address these pitfalls. Phase names follow the target-feature names in `.planning/PROJECT.md`; phase numbering will be finalized when `.planning/ROADMAP.md` is regenerated for v1.3.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Determinism drift from solver iteration order | Reachability and root semantics phase (install gate); enforced in every solver-introducing phase | Determinism gate fixture: same inputs → byte-identical observed JSON across 10 runs with shuffled provider order |
| 2. Cache invalidation gaps for cross-family dependencies | Shared semantic graph and unified call-graph solver phase (design dependency index); cache and solver budgets phase (consolidate) | Single-input mutation fixture per new fact family; per-family negative (preserved) fixture |
| 3. Recall-via-flooding | Adaptation model layer phase (precision floors before adaptation wins); benchmark promotion gate phase (hard gate on precision floor) | "Flooding" synthetic fixture rejected by gate; F-score β=0.5 tracked alongside F1; precision floor never crossed |
| 4. Benchmark-label leakage | Adaptation model layer phase (sandbox + validation + held-out) | Prompt-sanitizer fixture; held-out subset report column; label-overlap rejection fixture |
| 5. Go FFI lifecycle | Go semantic frontend phase (process boundary, timeout, version pinning) | SIGTERM-cleanup fixture; version-skew fixture; long-running sidecar fixture; cancellation propagation fixture |
| 6. Token-set memory blowup | JS/TS function-token propagation solver phase and JS/TS object/property model phase (per-family budgets); cache and solver budgets phase (consolidate) | Memory-ceiling fixture per solver; `BudgetExceeded` non-zero fixture for runaway inputs |
| 7. Span identity drift | JS/TS function and callsite inventory phase (renderer + coverage gate) | `>99%` Jelly span-identity coverage gate; CRLF/LF cross-platform fixture; ambiguous-construct fixture per Oxc form |
| 8. Public/private API leak | All v1.3 phases (rule enforced from start); CI gate added in benchmark identity phase | Public-surface-leak CI fixture: external rule crate compiles against `polint::sdk::prelude::*` and reaches zero v1.3 solver types |
| 9. Mode conflation (reachable vs. full) | Reachability and root semantics phase (per-suite scoring mode); benchmark promotion gate phase (enforce in gate) | Suite manifest `scoring_mode` required; per-suite root policy fixture |
| 10. Provenance loss across solver | Shared semantic graph and unified call-graph solver phase (define `DerivedEdgeProvenance` contract); every subsequent solver phase extends | `polint explain` completeness fixture for derived edges; deletion-invalidates-derived property test |
| 11. Cross-cutting precision regression | Shared semantic graph and unified call-graph solver phase (introduce per-language policy traits); Go RTA, JS/TS token solver phases (enforce per-language gates); benchmark promotion gate phase (per-language deltas in report) | Polyglot canary fixture; per-language delta required in promotion gate; per-language solver config fixture |
| 12. Hidden coupling between abstract-domain kernel, summary kernel, and unified solver | Shared semantic graph and unified call-graph solver phase (close-set input contract, iteration cap) | Dependency-cycle detection fixture; iteration-cap fixture; provider DAG snapshot includes solver |

## Sources

- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/.planning/PROJECT.md` — current v1.2 substrate and v1.3 milestone definition (HIGH confidence — primary source).
- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — sections 1–11 (identity, reachability, Go frontend, Go RTA, JS/TS inventory, JS/TS scope/binding, JS/TS token solver, JS/TS object model, adaptation, unsupported taxonomy, cache/budget) and the adaptation boundary section explicitly rejecting label leakage and flooding (HIGH confidence — explicit source for v1.3 architecture and risks).
- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/research/ROADMAP.md` — v1.2 PR sequence (PRs 1–22) and design consequences for kernel/extension boundaries (HIGH confidence — v1.2 substrate context).
- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/research/evaluation-harness/STANDARD.md` — adaptation rule (no expected labels, no answer keys), reporting requirements, scope rule limiting scored suites to Go and TS/JS (HIGH confidence — explicit anti-leakage policy).
- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/research/evaluation-harness/decisions/decision-log.md` — D1–D6 decisions establishing external-benchmark-first strategy, hidden/internal-first SDK discipline, evaluation precedes implementation (HIGH confidence).
- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/.planning/ROADMAP.md` — v1.2 archived phase list (Phases 20–41) used for phase-name precedent (HIGH confidence).
- `/Users/emilwareus/conductor/workspaces/exlint/albuquerque/.planning/research/archive-v1.0/PITFALLS.md` — v1.0 pitfall precedents (determinism, cache unsafety, heuristic over-claims) extended for v1.3 solver context (HIGH confidence — internal precedent).

---
*Pitfalls research for: polint v1.3 Graph Engine Precision (shared semantic graph + unified call-graph solver, Go RTA, JS/TS token+object models, adaptation, budgets, taxonomy)*
*Researched: 2026-05-27*
