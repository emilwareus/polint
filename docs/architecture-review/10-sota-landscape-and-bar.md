# 10 — The Bar: SOTA Landscape and What "Most Capable" Requires

**Purpose:** define, from outside the codebase, what capability set a 2026 static-analysis engine must
clear to be credibly "most capable," which architectural properties that set forces, and where the
genuine white space is.

**Epistemic note:** figures below are from my training knowledge of these systems' public
documentation and papers, not verified against live sources in this session (the web-research agents
for this document hit the session limit). Treat *architectural characterizations* as high confidence
and *specific numbers* as approximate — marked `~`. Anything load-bearing for a decision should be
re-verified before it goes into a plan.

---

## (a) The landscape

| System | Core model | Extension language | Incrementality | Soundness posture | Key limit | The one idea worth stealing |
|---|---|---|---|---|---|---|
| **CodeQL** | Relational DB built by per-language *extractors*; queries are Datalog-with-classes, evaluated bottom-up with magic sets | QL | Rebuild DB per commit; cached query results | Deliberately unsound, tuned for precision; explicit "MaD" (models-as-data) for library behavior | DB build is minutes-to-hours; queries can blow up | **The extractor/database split.** Frontends produce relations; analyses are queries. Nothing in the analysis layer knows what a language is. |
| **Semgrep** | Syntactic pattern matching over per-language ASTs; Pro adds interfile/interprocedural taint | YAML patterns + taint mode | Per-file, trivially parallel | Explicitly shallow; optimizes for FP rate and speed | Interprocedural depth is bounded; not a whole-program engine | **Rule ergonomics and the registry.** Time-to-first-rule measured in minutes, and rules compound across users. |
| **Meta Infer** | Bi-abduction / separation logic; **compositional function summaries** | OCaml (checkers) | Genuinely incremental: summaries are per-function and reused | Sound-ish per-analysis, explicitly bounded | Deep language/build coupling; OCaml barrier | **Compositional summaries.** Analyze a function once, reuse everywhere. This is what makes diff-time analysis possible at monorepo scale. |
| **Joern / CPG** | Code Property Graph (AST+CFG+PDG unified); graph traversal queries | CPGQL / Scala | Rebuild graph | Heuristic; precision varies by language frontend | Precision and memory at scale | **One unified graph.** Every analysis is a traversal over the same node/edge vocabulary. |
| **SCIP / LSIF / Kythe / Glean** | Index *format*, not an analyzer — indexers emit facts, servers query them | N/A (data format) | Per-file/per-unit indexes, merged | N/A | Read-only; no analysis, only navigation | **Standard symbol identity (monikers).** Cross-repo, cross-language, stable across edits. Someone else's frontend can feed you. |
| **rust-analyzer / Salsa** | Demand-driven memoized query graph with red-green invalidation | N/A (IDE) | Best-in-class; sub-second on edits | N/A | Not an analysis engine | **Demand-driven queries.** Never compute what nobody asked for; invalidate by dependency, not by timestamp. |
| **Doop / Souffle** | Andersen-style points-to expressed as Datalog, compiled to parallel C++ | Datalog | Batch | Sound-ish, context-sensitivity is a dial (`1-call`, `2-obj`, …) | Batch-only; memory-hungry | **Context sensitivity as configuration.** Same rules, different precision/cost tradeoff by flag. |
| **WALA / SootUp / OPAL** | JVM analysis frameworks with pluggable IFDS/IDE solvers | Java/Scala | Batch | Configurable | JVM-only; research-grade ergonomics | **A generic IFDS/IDE solver.** Write a taint analysis as a set of flow functions, get an interprocedural solver for free. |
| **Ruff / oxc / Biome** | Per-file AST lint, Rust, aggressively parallel | Rust plugins (in-tree) | Per-file | N/A | Single-file only | **Speed as a feature.** Sub-second on large repos changes *when* the tool runs, which changes what it's for. |
| **SonarQube / Coverity / Fortify** | Commercial SAST, broad language matrix, proprietary interprocedural | Limited custom rules | Server-side incremental | Tuned for compliance | FP rates; slow; closed | **Breadth as moat.** Language coverage is the buying criterion, not depth. |

**The tiering insight (Google Tricorder):** the systems that actually get used at scale run *several*
analyses at different cost tiers — instant per-file lint on keystroke, per-commit interprocedural,
nightly whole-program — rather than one analysis at one speed. polint's `run_*_pipeline` boolean gates
are an unwitting first draft of this; it should become explicit.

---

## (b) The capability bar

To be credibly "most capable" in 2026, an engine needs all of:

**Analysis**
1. Interprocedural taint (source → sanitizer → sink) with a **replayable path**, not just a verdict.
2. A whole-program call graph with declared precision — and *field sensitivity* for object-oriented
   and JS-style code, since that is where imprecision actually comes from.
3. Framework awareness: routes, DI containers, ORMs, RPC boundaries, serialization. Real code's
   entrypoints are not `main`. This is where CodeQL's MaD and Semgrep's Pro rules earn their keep.
4. **Explicit unsoundness accounting** — the engine reports where it gave up (dynamic dispatch,
   reflection, missing setup, budget exceeded), per finding.
5. Cross-language flow for polyglot repos (TS frontend → Go backend over an HTTP contract). Nobody
   does this well. It is the clearest open problem.

**Engineering**
6. Editor-latency incremental re-analysis on edit (sub-second for local queries).
7. Monorepo scale: 10M+ LOC without OOM, with a bounded memory budget.
8. Deterministic, cacheable, shareable results (CI cache; ideally remote).
9. Low false-positive rate — the actual adoption gate, above all capability claims.

**Product**
10. Custom rules that a non-expert (or an LLM) can write in minutes and *verify*.
11. Rules that compound: shareable, versioned, testable.
12. Machine-readable output with provenance, suitable for an agent to act on and for a human to audit.
13. Published, reproducible benchmarks — capability claims that a third party can falsify.

polint today clears **10** outright, has real substance on **4** (built but switched off) and **12**
(built but stripped), partial on **6/8**, and does not clear the rest. That is a respectable position
for a 3-month-old codebase, and an honest one.

---

## (c) Required architectural properties

Each derived from the bar above, and each is a constraint on the target architecture:

| Requirement | Forces this property |
|---|---|
| #1, #2 (interprocedural, whole-program) | **Compositional summaries.** Not whole-program-in-RAM. This is the difference between Infer's model and a batch solver, and it is the *only* known way to get #6 and #7 simultaneously. |
| #3 (frameworks) | **Models as data, not code.** Framework knowledge must be a loadable artifact, or every framework is an engine release. |
| #4 (unsoundness) | **Unknown as a first-class lattice value**, propagated and attributable — not a log line. |
| #5 (cross-language) | **A language-neutral fact/IR layer** that no analysis can see through to a specific language. |
| #6 (editor latency) | **Demand-driven queries** with dependency-tracked invalidation. Eager pipelines cannot get here by optimization. |
| #7 (scale) | **Persistent store + eviction.** The working set must be decoupled from repo size. |
| #8 (determinism) | Content-addressed keys over every input, including config and rule options. |
| #11 (compounding rules) | **A distribution unit and a stability contract.** |
| #13 (falsifiable claims) | **Benchmarks wired to CI gates**, not to a README table. |

Read against the current codebase, four of these are already *architecturally* decided the wrong way:
eager whole-program `AnalysisDb`, no summary reuse across runs, no fact persistence, and language
enums threaded through 129 files. Those are the load-bearing items in the target architecture doc.

---

## (d) Three defensible wedges

Incumbents are not weak everywhere. These three are places where polint's actual differentiators line
up with something structurally hard for them to copy.

### 1. Rules-as-code that an LLM can write *and the compiler can verify*

Every incumbent's custom-rule story is a DSL — QL, YAML, CPGQL. An LLM writing QL produces plausible
queries that silently return nothing. An LLM writing a polint rule either compiles or does not, and
the capability planner refuses to run a rule whose facts are unavailable. **The type checker is the
verifier**, and it runs before any human sees output.

Why incumbents can't copy it: their query languages are their moat and their compatibility surface.
Semgrep cannot make YAML type-check; CodeQL cannot make QL fail fast on a wrong join. polint's choice
of "boring host language + typed views" looks like a limitation and is actually the wedge.

**Requires:** compile time to stay tolerable (currently unmeasured — see doc 06/07), and rule packs to
be shareable (currently impossible — see doc 07).

### 2. Provenance-first findings

The bar item nobody clears well is #4 + #12 together: *every finding carries a replayable, bounded,
truncation-aware evidence path with declared precision and named unknowns*. CodeQL has path queries
but they are expensive and query-specific. Semgrep has shallow traces. Nobody exposes "here is
exactly where I gave up and why" as a structured, per-finding contract.

polint has already built this — `analysis/evidence/`, 4,335 lines, with unknown reasons, omitted
regions, replay keys, and rendering budgets — and then strips it at the boundary
(`diagnostics/mod.rs:1136-1139`). This is the single largest built-but-unshipped asset in the
repository.

Why it matters more in 2026 than it did in 2020: an agent consuming findings needs to know *why* to
decide whether to act. A finding without provenance is a coin flip an LLM will confidently
rationalize. Provenance is the difference between "AI triage" and "AI hallucination laundering."

### 3. Policy, not vulnerability

Every incumbent competes on CWE coverage. polint's framing — *your* repo's rules, your architecture
boundaries, your conventions — is a different buyer and a different moat: it is unwinnable by a
central rule registry, because the rules are about facts only this repo knows. Combine with (1) and
the loop becomes: an agent reads the codebase, proposes a policy, writes it as a verified rule, and
the rule then constrains the agent. That is a genuinely new product shape, and it is adjacent to what
the owner has already built rather than a pivot.

**The honest caveat:** wedges 1 and 3 are product wedges that need only modest analysis depth. Wedge 2
needs real interprocedural analysis to be worth anything. The temptation will be to chase depth
because it is intellectually satisfying; the leverage is in shipping 1 and 3 on top of *adequate*
depth, and letting depth compound behind them.

---

## (e) Anti-patterns to design away from

| Incumbent failure | What it costs them | Design rule for polint |
|---|---|---|
| CodeQL's DB build time | Cannot run on keystroke; adoption is CI-only | Never require a full rebuild for a local question. Demand-driven from day one. |
| Semgrep's shallow interprocedural | Misses real multi-hop flows; users don't know when | If depth is bounded, *say so per finding*. Bounded + honest beats deep + silent. |
| Infer's language coupling | Each new language is a major project | Keep the analysis core blind to language. This is currently violated in 129 files. |
| Joern's precision | Findings need manual triage; trust erodes | Precision is a product feature. Gate merges on measured FP rate, not on feature completion. |
| SCIP's read-only nature | Great index, no analysis | Consume external indexes where they exist — don't rewrite frontends you can import. |
| SAST incumbents' FP rates | The single biggest cause of tool abandonment | An FP budget in CI, enforced. |
| Everyone's benchmark opacity | Nobody believes anyone's numbers | Publish reproducible benchmarks with a public harness. This is cheap and nobody does it. |

---

## (f) What polint must NOT try to be

This section exists because the failure mode for a 267k-LOC, 3-month-old, one-person-plus-agents
codebase aiming at "most capable" is not building too little. It is building all of it.

1. **Not a CWE/SAST vendor.** Do not chase language-count parity or CVE coverage. That is a sales
   motion with a 10-year head start and a compliance-checkbox buyer.
2. **Not a query language.** The ADR already decided this
   (`research/agent-rule-authoring/decisions/001-typed-rust-rules-not-dsl-first.md`) and the decision
   is right. Building a QL clone means building a compiler, an optimizer, a package system, a
   debugger, and a compatibility promise — none of which is the product.
3. **Not sound.** Soundness is a research goal that trades directly against FP rate and speed. Be
   *honest* instead: declare precision per finding, name the unknowns. Honest-and-bounded is a
   stronger position than sound-in-principle.
4. **Not a whole-program batch analyzer.** That road ends at CodeQL's build times. Summaries and
   demand-driven queries, or nothing.
5. **Not its own frontend for every language.** Writing a production TS type-checker or a C++
   frontend is a multi-year project each. Consume tsc/gopls/SCIP/LSP where they exist.
6. **Not an IDE.** Editor latency is a requirement on the engine, not a mandate to ship a plugin
   suite.

The strategic read: polint's realistic path to "most capable" is **not** matching CodeQL on depth or
Semgrep on breadth. It is being the only engine where a repository's own policies are executable,
verified by a compiler, explained by provenance, and fast enough to run on every keystroke — and then
letting analysis depth accrue underneath that, one honestly-measured capability at a time.
