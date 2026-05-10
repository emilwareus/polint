# Capability Fulfillment Research

## Purpose

polint should not shrink its capability promises to match the current
implementation. It should fulfill them.

The current `Capabilities` type in
[`crates/polint/src/core/mod.rs`](../crates/polint/src/core/mod.rs) is the
public promise that repo-local rules can declare the analysis facts they need.
Supported capabilities have typed SDK views and adapter-produced facts. Others
are reserved future views that should stay unavailable until real facts exist.

This document ranks the most impactful work to make those promises real.

The actionable checkbox roadmap derived from this research lives in
[`roadmap/00_ROADMAP.md`](roadmap/00_ROADMAP.md).

## Product Direction

Go and TypeScript/JavaScript are the first target languages for full capability
coverage. They should prove the complete model before polint expands the same
promise to Python, Java, and other languages.

That does not mean the design should be Go/TS-specific. The engine should keep a
stable cross-language fact model and let each language adapter fill as much of
that model as it can. New languages should not require rule authors to learn a
new engine. They should plug into the same capabilities, `RuleCtx` accessors,
diagnostics, cache, graph, and CI outputs.

The desired long-term shape:

- Go and TS/JS: full feature coverage first.
- Python, Java, and later languages: added through the same adapter contract.
- Language-native analyzers are allowed when they materially improve precision.
- Some setup from the user or coding agent is acceptable when it unlocks deeper
  facts, such as type info, module resolution, or coverage import.
- polint should prefer owning the common analysis model and rule SDK, even when
  a language adapter delegates parsing or semantic facts to existing language
  tooling.

## Current Gap

`Capabilities` looks like a contract: a rule declares a capability and polint
computes/provides that fact family.

Today, capabilities are mostly descriptive metadata. The runner loads files,
runs language adapters, and then rules read whatever facts the adapters
harvested. That means the API suggests capability-gated analysis, but the engine
does not yet use capabilities as the planning input for analysis work.

The high-level principle:

> Keep the promise. Make each capability backed by public facts, public SDK
> accessors, docs, cache participation, and external-consumer tests.

## Multi-Language Implications

Capability fulfillment should be designed around a matrix:

| Capability | Go | TS/JS | Python | Java | Notes |
|---|---|---|---|---|---|
| syntax/facts | target full | target full | future | future | Common source, span, function, import, literal facts. |
| resolved imports | target full | target full | future | future | Needs language-specific module/package rules. |
| CFG | target full | target full | future | future | Common graph API; adapter-specific builders. |
| call graph | target full | target full | future | future | Direct calls first, resolved calls later. |
| dataflow | target full | target full | future | future | Built on CFG, symbols, and call graph facts. |
| coverage | target full | target full | future | future | Ingest existing coverage outputs per ecosystem. |
| symbols/references | target full | target full | future | future | Common symbol IDs; language-native help allowed. |
| test metrics | target full | partial/target | future | future | Test semantics are language/ecosystem-specific. |

The table should become a living status section once implementation starts.
Adding a language means explicitly choosing its supported capability tier rather
than accidentally implying parity.

## Analyzer Ownership Strategy

polint should own:

- the rule SDK
- the fact types
- stable IDs and spans
- capability planning
- cache keys and invalidation
- diagnostics and machine output
- cross-language graph/query APIs

Language adapters may use:

- in-house syntax extraction built on parser crates
- language-native tools when they provide better semantic facts
- optional sidecars invoked by polint
- user-provided setup files or generated metadata

Examples:

- Go can start with tree-sitter facts and later use `go list`, `go/packages`, or
  `go test -coverprofile` inputs for resolution, types, and coverage.
- TS/JS can start with Oxc facts and use `oxc_resolver`, `tsconfig.json`, and
  Istanbul/LCOV inputs.
- Python could use a Rust parser for syntax, then optionally consume
  `pyproject.toml`, import metadata, coverage.py XML/JSON, or Python-side tools
  for deeper semantics.
- Java could use a parser for syntax, then optionally consume Maven/Gradle
  project metadata, classpaths, JaCoCo reports, or language-server/compiler
  output for symbols and types.

The important boundary is that rule authors consume polint facts, not raw
language-tool output. External tools are implementation details behind adapters.

## Setup Expectations

It is acceptable for deep analysis to require setup when the setup is explicit
and machine-checkable.

Examples:

- TS/JS resolved imports may require `tsconfig.json` or package manager metadata.
- Go package resolution may require `go list` to run in the module.
- Java symbols may require a Gradle/Maven classpath.
- Python import resolution may require a virtual environment or lockfile.
- Coverage facts require a coverage report path in `.polint.toml` or CI config.

Rules should be able to distinguish:

- capability unavailable because the adapter does not implement it
- capability requested but setup is missing
- capability requested and setup exists, but some facts remain unresolved

Those states should become diagnostics or structured warnings, not silent empty
fact sets.

## Difficulty Scale

| Rating | Meaning |
|---|---|
| S | Small: isolated public type/helper work, usually days. |
| M | Medium: one adapter plus tests/docs, roughly one to two weeks. |
| L | Large: shared model plus multiple adapter changes, roughly several weeks. |
| XL | Extra large: semantic/project-wide analysis with setup and caching concerns. |
| XXL | Multi-phase: language semantics, project model, precision tiers, and long-term hardening. |

Ratings assume production-quality implementation: public SDK design, docs,
external-consumer tests, cache behavior, and CLI explain/debug surfaces.

## Difficulty Ratings And Build Methods

### 1. Capability-Driven `AnalysisPlan`

Difficulty: **L**

Why:

- The model is conceptually simple, but it touches runner orchestration,
  adapters, cache keys, diagnostics, and tests.
- It must be introduced without regressing today's all-facts behavior.

Concrete build method:

1. Add `AnalysisPlan` and `LanguagePlan` structs in `core`.
2. Add `Capabilities::union` and a deterministic encoder for the plan.
3. In `runner::analyze_and_run`, build the plan from enabled rules before
   loading adapters.
4. Thread the plan through Go and TS/JS adapter entrypoints.
5. Keep parser diagnostics on by default.
6. Gate optional harvesters behind plan flags.
7. Include the encoded plan in `rule_hash` or a new cache digest component.
8. Add external temp-repo tests proving that changing a rule capability changes
   the analysis/cache plan.

Useful references:

- Go `packages.Config.Mode` already models capability-style loading with
  different `Need*` bits.
- Oxc semantic analysis already separates symbol, reference, module, and CFG
  work conceptually.

### 2. General CFG Facts

Difficulty: **L for TS/JS, XL for Go, L later for Python, L/XL later for Java**

Why:

- CFG shape is easy to define badly and hard to stabilize.
- TS/JS has the most leverage because `oxc_semantic` already advertises optional
  CFG construction.
- Go currently has branch obligations but not general basic blocks.

Concrete build method:

1. Add public graph types: `ControlFlowGraph`, `BasicBlock`, `CfgEdge`,
   `CfgNodeId`, and `CfgEdgeKind`.
2. Add `Cfg<'_>::for_function(function_id) -> Option<&ControlFlowGraph>`.
3. For TS/JS, adapt Oxc semantic CFG output into polint's graph model.
4. For Go, start from existing branch extraction and expand to entry, exit,
   sequential statement, branch, loop, switch, defer, and panic/return edges.
5. For Python later, build from Python AST statements: `If`, `For`, `While`,
   `Try`, `With`, `Return`, `Raise`, `Break`, `Continue`.
6. For Java later, use `JavacTask.parse()` plus tree scanners, or JavaParser,
   to produce the same graph model.
7. Store CFG facts in cache only when requested by the plan.
8. Document precision: syntax-level first, no dataflow or type-sensitive
   dispatch.

### 3. Coverage Facts Import

Difficulty: **M for line coverage, L for branch/function mapping, XL for exact cross-language branch mapping**

Why:

- Ingesting files is straightforward.
- Mapping external coverage ranges to polint spans and branch facts is the hard
  part.

Concrete build method:

1. Add config sections such as `[coverage.go]`, `[coverage.ts]`,
   `[coverage.python]`, and `[coverage.java]` with report paths.
2. Add public facts: `LineCoverageFact`, `BranchCoverageFact`,
   `FunctionCoverageFact`, `CoverageSource`, and `CoveragePrecision`.
3. Add `CoverageFacts<'_>::for_file(file_id)`,
   `CoverageFacts<'_>::for_function(function_id)`, and
   `CoverageFacts<'_>::for_branch(branch_id)`.
4. Parse Go text coverprofiles generated by `go test -coverprofile`; also
   support converted integration coverage from `go tool covdata textfmt`.
5. Parse LCOV for TS/JS and any ecosystem that can emit LCOV.
6. Parse coverage.py JSON/XML for Python later.
7. Parse JaCoCo XML for Java later.
8. Normalize report paths to repo-relative paths using the same file discovery
   and path context machinery as analysis.
9. Map line intervals first; add branch/function mapping where adapter facts
   provide stable spans.
10. Emit setup diagnostics when a rule requests coverage but the report path is
    missing.

### 4. Resolved Imports And Module Graph

Difficulty: **M for TS/JS, M/L for Go, L for Python later, XL for Java later**

Why:

- TS/JS has a direct resolver crate already in the stack.
- Go package loading is available through Go tooling, but build tags/modules
  make correctness setup-sensitive.
- Java resolution depends on classpath/build-system setup.

Concrete build method:

1. Add `ResolvedImportFact` with `from_file`, `import`, `target_file`,
   `target_package`, `resolution_status`, and `unresolved_reason`.
2. Add `ModuleGraph` with file/package/module nodes and import edges.
3. Add typed SDK views such as `ResolvedImports<'_>` and
   `ModuleGraphFacts<'_>`.
4. For TS/JS, use `oxc_resolver::ResolveOptions` with `tsconfig`, extensions,
   condition names, main fields, and package exports/imports settings.
5. For Go, use `go/packages.Load` with import/module modes, then map package
   IDs and `GoFiles` back to polint files.
6. For Python later, combine syntax imports, repo roots, `pyproject.toml`,
   configured virtualenv/interpreter path, `importlib` behavior, and installed
   distribution metadata where available.
7. For Java later, consume Maven/Gradle classpath setup and resolve packages
   through javac or JavaParser symbol solver.
8. Preserve unresolved imports as facts with explicit reasons.

### 5. Resolved Call Graph

Difficulty: **M for direct syntactic call edges, XL for resolved Go/TS call graph, XXL for precise dynamic-language call graph**

Why:

- Direct calls can be harvested from existing function/call extraction.
- Resolved calls require symbol/reference and import/module resolution.
- Python and JavaScript dynamic dispatch must expose confidence instead of fake
  precision.

Concrete build method:

1. Add `CallFact` or `CallEdgeFact` with `caller`, `callee_text`, `span`,
   `resolved_target`, `resolution_status`, and `confidence`.
2. Populate direct syntactic call facts from current `FunctionFact::calls`, but
   keep spans and call expression kind.
3. For TS/JS, use Oxc semantic symbols/references plus resolved imports to link
   direct function/class/member calls where possible.
4. For Go, use `go/packages.Load` with `NeedSyntax`, `NeedTypes`, and
   `NeedTypesInfo`; resolve identifier and selector calls through `types.Info`.
5. For Java, use `JavacTask.analyze()` and `Trees.getElement(TreePath)`, or
   JavaParser's symbol solver, to map method invocations to elements.
6. For Python later, start with lexical/direct call names and import-resolved
   module functions; mark attribute/dynamic calls as unresolved or low
   confidence.
7. Add `CallGraph<'_>::edges()` and `CallGraph<'_>::calls_from(function_id)`.

### 6. Symbols And References

Difficulty: **M/L for TS/JS, L for Go, L/XL for Java, XL/XXL for Python precision**

Why:

- TS/JS and Go have existing semantic tooling.
- Java can use compiler APIs or JavaParser, but setup is classpath-sensitive.
- Python has compiler symbol tables, but exact runtime targets are dynamic.

Concrete build method:

1. Add shared types: `SymbolFact`, `ReferenceFact`, `DefinitionFact`,
   `SymbolId`, `ReferenceKind`, `SymbolKind`, and `SymbolPrecision`.
2. Define stable symbol keys from language, package/module path, file, lexical
   owner, name, and span.
3. Add typed SDK views such as `Symbols<'_>` and `References<'_>`, with query
   methods like `References<'_>::to(symbol_id)` and
   `Symbols<'_>::definition(symbol_id)`.
4. For TS/JS, adapt Oxc semantic symbol tables and reference tracking.
5. For Go, use `go/packages.Load` with typed syntax and `TypesInfo` to map defs,
   uses, selections, and packages.
6. For Python later, start with `ast` plus `symtable` for lexical scopes; use
   import resolution and optional type-checker metadata later.
7. For Java later, use javac `JavacTask`/`Trees` or JavaParser symbol solver to
   map identifiers and method invocations to elements.
8. Store unresolved/ambiguous references explicitly.
9. Make call graph resolution consume these facts rather than inventing a
   parallel symbol model.

### 7. Test Suite Metrics

Difficulty: **M for Go, M/L for TS/JS, M/L for Python later, M/L for Java later**

Why:

- This is mostly pattern extraction and aggregation.
- The hard part is keeping framework-specific semantics behind a common fact
  model.

Concrete build method:

1. Add `TestMetricFact` and `RelatedTestEvidence` over existing `TestFact`.
2. Add `TestSuiteMetrics<'_>::for_file(file_id)` and
   `TestSuiteMetrics<'_>::for_function(function_id)`.
3. For Go, aggregate existing `TestFact` fields: assertion count, subtests,
   table rows, evidence terms, and related production files.
4. For TS/JS, detect Jest/Vitest/Mocha-style `describe`, `it`, `test`,
   assertion calls, table tests, and snapshot calls from call facts.
5. For Python later, detect pytest/unittest functions/classes, parametrization,
   fixture use, and assertions from AST.
6. For Java later, detect JUnit/TestNG annotations, assertions, parameterized
   tests, and related class names.
7. Keep framework-specific evidence fields optional; expose normalized metrics
   for rules.

### 8. Python Adapter

Difficulty: **M for syntax facts, L for imports/tests/coverage, XL/XXL for symbol and call precision**

Concrete build method:

1. Start with syntax, functions/classes, imports, literals, and basic branches.
2. Prefer a Rust parser or tree-sitter for baseline analysis if it keeps the
   engine self-contained.
3. Optionally use Python's own `ast` and `symtable` through a sidecar when exact
   CPython grammar/symbol behavior is worth the setup cost.
4. Support configured interpreter/virtualenv for import resolution.
5. Consume coverage.py JSON/XML/LCOV output for coverage facts.
6. Add pytest/unittest test metrics.
7. Clearly mark dynamic call targets and runtime imports as unresolved unless
   setup provides stronger evidence.

### 9. Java Adapter

Difficulty: **L for syntax facts, XL for resolved imports/symbols/calls, M for coverage import**

Concrete build method:

1. Start with syntax, packages, classes, methods, imports, literals, and basic
   branches.
2. Choose between a self-contained parser path and a javac/JavaParser semantic
   path per capability.
3. For semantic facts, require setup that provides a Maven/Gradle classpath or
   compile command.
4. Use `JavaCompiler`/`JavacTask`/`Trees` when leaning on JDK-native analysis.
5. Use JavaParser plus its symbol solver if embedding JVM-side tooling is more
   practical.
6. Parse JaCoCo XML for coverage.
7. Detect JUnit/TestNG-style tests and assertions for test metrics.

## Priority 1: Capability-Driven Analysis Plan

Make `Capabilities` operational.

Add an `AnalysisPlan` built from the enabled rules before parsing. The plan
should include:

- requested languages
- requested fact families
- requested graph models
- requested external inputs such as coverage files
- required setup probes per language

Then adapters should harvest according to that plan. Parser diagnostics can
still run by default, but optional fact families should be driven by declared
capabilities.

Cache keys must include the resolved analysis plan once it affects harvested
facts. Otherwise changing capabilities could incorrectly reuse stale cached
facts.

Impact:

- makes `Capabilities` truthful
- improves large-repo performance
- gives future capabilities a concrete implementation contract
- gives new languages a clear adapter target

## Priority 2: General CFG Facts

Fulfill `Cfg<'_>` with an honest intra-procedural control-flow model.

Add public facts and accessors such as:

- `ControlFlowGraph`
- `BasicBlock`
- `CfgEdge`
- `Cfg<'_>::for_function(function_id)`

Start syntax-level for Go and TS/JS and reach full coverage there first. The
first version does not need type semantics, but it must be a real graph rather
than a placeholder. Later languages should implement the same graph API through
their adapters.

Impact:

- unlocks branch-shape rules beyond Go-specific branch obligations
- provides a foundation for dataflow
- turns an existing capability into a usable rule-author primitive

## Priority 3: Coverage Facts Import

Fulfill `CoverageFacts<'_>` by ingesting external coverage reports.

Start with formats users already have:

- Go `coverprofile`
- Istanbul / LCOV for TS/JS
- coverage.py XML/JSON for Python later
- JaCoCo XML for Java later

Map coverage back to files, functions, branches, or lines where possible:

- `LineCoverageFact`
- `BranchCoverageFact`
- `FunctionCoverageFact`
- `CoverageFacts<'_>::for_file`
- `CoverageFacts<'_>::for_branch`

The API should make precision explicit. A coverage fact can say whether it came
from line coverage, branch coverage, or a heuristic mapping.

Go and TS/JS coverage import should come first. Python and Java can reuse the
same public facts once their report parsers and path mapping are available.

Impact:

- enables policies like "new error branches need test coverage evidence"
- connects static facts to CI/runtime evidence
- makes the existing `CoverageFact` model useful to real rules

## Priority 4: Resolved Call Graph

Fulfill `CallGraph<'_>` in stages.

Start with conservative direct-call facts:

- caller `FunctionId`
- callee text
- call span
- optional resolved target
- confidence / precision tier

Then improve resolution as symbol and import resolution land. The public API
should expose uncertainty instead of pretending every call target is exact.

Go and TS/JS should be the full-coverage targets. Python and Java can start with
direct calls and graduate to resolved calls when module/classpath setup is
available.

Impact:

- enables architectural rules over function usage
- builds on existing `FunctionFact::calls`
- gives users a real graph model while preserving honesty about precision

## Priority 5: Resolved Imports And Module Graph

Strengthen `imports()` with resolution facts.

Add:

- `ResolvedImportFact`
- `ModuleGraph`
- `ResolvedImports<'_>`
- `ModuleGraphFacts<'_>`

Start with TS/JS through `oxc_resolver`, then add Go package/module resolution.
Keep the existing syntactic `ImportFact` because unresolved imports are still
useful evidence. Later language adapters should map Python imports and Java
packages/classes into the same module graph model instead of adding unrelated
language-specific graph APIs.

Impact:

- unlocks common repo-local policies such as layer boundaries
- supports call graph and symbol resolution
- gives users file/package targets instead of only import strings

## Priority 6: Symbols And References

Add the stable building blocks for precise rules.

Add facts/accessors such as:

- `SymbolFact`
- `ReferenceFact`
- `DefinitionFact`
- `Symbols<'_>`
- `References<'_>::to(symbol)`

This is the foundation for precise call graphs, ownership rules, exported API
rules, dead-code style checks, and security rules.

The engine should own the public symbol/reference model. Adapters can fill it
from Oxc, Go tooling, Python metadata, Java compiler/language-server output, or
in-house extraction.

Impact:

- moves users beyond string matching
- makes policies over definitions and usages possible
- creates the path toward type-aware analysis

## Priority 7: Test Suite Metrics

Fulfill `TestSuiteMetrics<'_>` with first-class metrics beyond raw `TestFact`.

Add metrics such as:

- per-file test counts
- per-function related test counts
- assertion density
- subtest and table-test shape metrics
- test naming/evidence coverage score

Expose via:

- `TestSuiteMetrics<'_>::for_file`
- `TestSuiteMetrics<'_>::for_function`

Go should get full coverage first because it already has `TestFact`. TS/JS test
metrics should follow with common Jest/Vitest/Mocha patterns. Python and Java
should map pytest/unittest and JUnit/TestNG-style facts into the same metric
model where possible.

Impact:

- makes test-quality policies easier to write
- turns current Go test facts into higher-level reusable building blocks
- supports agents reviewing whether risky code has meaningful tests

## Language Adapter Contract

Each language adapter should implement a common contract:

1. Declare supported capabilities and precision tiers.
2. Validate required setup for requested capabilities.
3. Harvest requested facts into the shared fact model.
4. Emit structured unsupported/setup diagnostics when a request cannot be met.
5. Preserve stable IDs and spans.
6. Participate in cache keys through the resolved `AnalysisPlan`.
7. Provide at least one external rule test for every promoted capability.

This lets Python, Java, and later languages join incrementally without weakening
the promise for Go and TS/JS.

## Capability Verification Contract

Every capability should have a verification checklist:

- public fact types, graph types, or typed SDK views
- at least one adapter producing the facts
- cache-key participation when capability changes harvested facts
- docs under `docs/facts/`
- at least one external generated-rule test consuming the capability
- no visible CLI command unless the capability is complete and valuable to users
- a language support matrix entry with precision/setup notes

This should become the release gate for new capabilities.

## Recommended Build Order

1. `AnalysisPlan` with language/capability/setup planning.
2. Go and TS/JS CFG facts.
3. Go and TS/JS coverage import.
4. Go and TS/JS resolved imports / module graph.
5. Go and TS/JS direct call graph facts.
6. Go and TS/JS symbols and references.
7. Go and TS/JS dataflow facts on top of CFG/symbol/call graph support.
8. Go and TS/JS richer test-suite metrics.
9. Python adapter with a declared subset, then expand toward parity.
10. Java adapter with a declared subset, then expand toward parity.

This order makes Go and TS/JS prove full feature coverage first, while the
engine stays shaped for more languages from the beginning.

## Research References

- Go package loading and typed syntax:
  <https://pkg.go.dev/golang.org/x/tools/go/packages>
- Go coverage profiles and `go tool cover`:
  <https://go.dev/doc/build-cover> and <https://go.dev/cmd/cover/>
- Oxc resolver options:
  <https://docs.rs/oxc_resolver/latest/oxc_resolver/struct.ResolveOptions.html>
- Oxc semantic analysis, symbols, references, and CFG:
  <https://docs.rs/crate/oxc_semantic/latest>
- Python AST:
  <https://docs.python.org/3/library/ast.html>
- Python import system:
  <https://docs.python.org/3/library/importlib.html>
- Python symbol tables:
  <https://docs.python.org/3/library/symtable.html>
- coverage.py reporting formats:
  <https://coverage.readthedocs.io/en/latest/commands/cmd_reporting.html>
- Vitest coverage providers/reporters:
  <https://vitest.dev/config/coverage.html>
- LCOV tracefile format:
  <https://manpages.debian.org/trixie/lcov/geninfo.1.en.html>
- Java compiler API:
  <https://docs.oracle.com/en/java/javase/21/docs/api/java.compiler/javax/tools/JavaCompiler.html>
- Java `JavacTask`:
  <https://docs.oracle.com/en/java/javase/21/docs/api/jdk.compiler/com/sun/source/util/JavacTask.html>
- Java compiler tree utilities:
  <https://docs.oracle.com/en/java/javase/21/docs/api/jdk.compiler/com/sun/source/util/Trees.html>
- JavaParser parser and symbol solver docs:
  <https://javaparser.org/getting-started.html>
- JaCoCo report formats:
  <https://www.jacoco.org/jacoco/trunk/doc/report-mojo.html>
