# 09 — Competitive Landscape

Standing reference: who else builds analysis substrates (CG/CFG/DF facts),
what they do well, what to steal, and where polint's wedge is.

## Engines / platforms

**CodeQL (GitHub/Microsoft)** — the reference "bring your own rules" engine:
extractors → immutable relational store on disk → Datalog-like QL,
semi-naive evaluation. Strengths: query ecosystem, multi-language depth,
disk-backed store (never OOMs the way we do). Weaknesses: whole-DB build cost
(minutes–hours), no incrementality in production (research retrofit:
arXiv:2308.09660), rules in a bespoke language, closed evaluator, licensing
(free for OSS only). Steal: relational store discipline, model packs
(framework summaries as data). Wedge: polint rules are plain Rust in-repo,
sub-second review-time goal, agent-native feedback.

**Semgrep** — syntax-first rules with cross-file/cross-function taint in Pro;
Assistant adds LLM triage (~95% agreement) and memories. Strengths: authoring
UX, speed, adoption. Weaknesses: shallow semantics vs real CG/points-to;
per-language taint depth varies. Steal: rule ergonomics, triage layer
economics (doc 07 §4). Wedge: real interprocedural facts with honesty labels.

**Joern** — open-source code property graph (CPG) for security research;
JoernTI ships verified neural type inference in production OSS
(arXiv:2310.00673) — the direct precedent for doc 07 §1. Weaknesses: memory-
heavy CPG, research-grade UX. Steal: CPG-as-queryable-graph ideas, JoernTI
integration pattern.

**Glean (Meta, OSS 2024)** — language-agnostic fact store (RocksDB, Angle
queries, stacked incremental DBs, billions of facts). Not an analyzer — the
storage layer ours should resemble (docs 03/04). Steal: unit-labeled
incrementality, schema/versioning discipline.

**Infer (Meta)** — compositional separation-logic analysis; the existence
proof for summaries-at-scale and diff-time deployment (CACM 2019).
Weaknesses: C/C++/Java/ObjC focus, hard to extend, no JS/TS/Go. Steal: the
architecture (docs 03/08); Pulse's under-approximate bias for review rules.

**Jelly (Aarhus)** — JS/TS callgraph research tool; our benchmark oracle and
closest algorithmic relative. PLDI'24 approximate interpretation (+12pp
recall) and ECOOP'24 indirection bounding are directly transferable.
Weakness: research tool, TypeScript implementation, no rule ecosystem.

**Sourcegraph SCIP / stack graphs (GitHub)** — code-nav indexes: SCIP
(compact, streamable index format; ~4-5× smaller than LSIF) and stack graphs
(incremental name resolution per file without whole-program work). Not
analysis engines, but the best references for **symbol-layer incrementality
and index formats** — relevant to module/symbol_graph persistence. Steal:
per-file incremental name binding (stack graphs' key idea), SCIP as an
interchange format for the symbol tier.

**SonarQube** — broad language coverage, shallow-to-medium semantics, strong
enterprise distribution; taint via commercial engines. Wedge: same as
Semgrep — depth + repo-local rules-as-code.

**Snyk Code (DeepCode)** — symbolic taint engine + ML-mined rules from OSS
commits; CodeReduce (arXiv:2402.13291) shows analysis-derived slicing making
LLM fixes 4× better. Steal: offline rule/spec mining pattern; slice-for-LLM
context idea (pairs with evidence bundles).

**ast-grep** — tree-sitter structural search/lint in Rust; fast, syntax-only.
Not a competitor on semantics; a competitor for "quick custom rule"
mindshare. Wedge: same authoring speed ambition, but with semantic facts.

**Google internal (Tricorder/Shipshape)** — the deployment playbook: diff-
time, latency budgets, usefulness gates, tiered analyzers (ICSE 2015).
Pattern already reflected in `polint review` and capability gating.

**Academic engines** — Doop (Datalog points-to, Java), SVF (C/C++, source of
our hash-consing reference), PyCG/Jarvis (Python CGs), WALA (Java/JS,
origin of ACG). Mine for algorithms, not products.

## Positioning summary

Nobody currently offers: **repo-local rules as plain typed code (Rust SDK) +
real semantic facts (CG/DF with honesty labels) + diff-time latency +
agent-native output + a queryable local semantic store that can later accept
trusted dependency summaries.** CodeQL has the facts but not the
latency/ergonomics/licensing; Semgrep has the ergonomics but not the depth;
Infer has the architecture but not the extensibility or languages; Glean has
the store but no analysis. The 2.0 architecture (tiers + summaries + demand +
verified ML) is the path to holding all five properties at once. A remote
dependency-summary registry may become a later network-effect asset, but the
first product moat is the local store plus Rust rules plus agent-facing
evidence.
