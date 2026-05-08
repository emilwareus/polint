# Capability Fulfillment Roadmap

This roadmap turns the capability research into an actionable build sequence.

The product direction is simple:

- Go and TypeScript/JavaScript reach full capability coverage first.
- Python, Java, and later languages join through the same adapter contract.
- polint owns the rule SDK, fact model, diagnostics, cache, graph APIs, and
  capability planning.
- Language adapters may use in-house analysis, parser crates, language-native
  tools, or sidecars when that gives better facts.

## Roadmap

- [ ] **1. Make capabilities drive analysis**
  ([technical plan](01_ENTRY_1_ANALYSIS_PLAN.md))

  Build an `AnalysisPlan` from enabled rules before parsing. This makes
  `Capabilities` a real contract instead of metadata. After this, a rule that
  asks for imports, CFG, coverage, symbols, or tests should cause polint to
  plan that work explicitly, validate required setup, and include the request in
  cache keys. This is the foundation for every later item.

- [ ] **2. Add real CFG facts for Go and TS/JS**
  ([technical plan](02_ENTRY_2_CFG_FACTS.md))

  Give rules a real per-function control-flow graph. This unlocks branch-shape,
  flow-sensitive, and future dataflow rules. Rule authors should be able to ask
  "what paths can this function take?" without parsing ASTs themselves. Go and
  TS/JS get this first so the graph model is proven before new languages inherit
  it.

- [ ] **3. Import coverage facts**
  ([technical plan](03_ENTRY_3_COVERAGE_FACTS.md))

  Let rules use CI coverage reports as evidence. This makes policies like
  "risky branches need coverage" possible. The goal is not to run tests inside
  polint; it is to consume coverage files teams already produce and map them
  back to source files, functions, branches, and lines with clear precision
  labels.

- [ ] **4. Resolve imports and build a module graph**
  ([technical plan](04_ENTRY_4_RESOLVED_IMPORTS_MODULE_GRAPH.md))

  Turn import strings into file/package/module relationships. This unlocks
  practical architecture and layer-boundary rules. Instead of matching
  `"../foo"` or `"net/http"` as raw text, rules should know what local file,
  package, module, or external dependency an import points to, and why an import
  could not be resolved when setup is missing.

- [ ] **5. Add direct call graph facts**
  ([technical plan](05_ENTRY_5_DIRECT_CALL_GRAPH.md))

  Expose caller-to-callee relationships with explicit confidence. This gives
  users a usable call graph without pretending every dynamic call is exact. The
  first version should support direct syntactic calls; later versions can attach
  resolved symbols when import and symbol facts are available.

- [ ] **6. Add symbols and references**
  ([technical plan](06_ENTRY_6_SYMBOLS_REFERENCES.md))

  Give rules definitions, references, and stable symbol IDs. This moves rules
  beyond string matching. Rule authors should be able to ask "where is this
  thing defined?" and "where is it used?" across files, with explicit precision
  tiers when a language or project setup cannot prove an exact answer.

- [ ] **7. Add reusable test-suite metrics**
  ([technical plan](07_ENTRY_7_TEST_SUITE_METRICS.md))

  Turn raw test facts into higher-level quality metrics such as related tests,
  assertions, subtests, table tests, and naming evidence. This lets teams write
  test-quality policies without every rule reimplementing framework detection
  and test evidence scoring.

- [ ] **8. Add Python through the adapter contract**
  ([technical plan](08_ENTRY_8_PYTHON_ADAPTER.md))

  Add Python with an explicit capability subset first, then expand toward parity
  using the same `RuleCtx` and fact model. Python should not become a separate
  product surface. It should reuse the same facts and diagnostics, while clearly
  marking dynamic imports/calls and optional virtualenv or interpreter setup.

- [ ] **9. Add Java through the adapter contract**
  ([technical plan](09_ENTRY_9_JAVA_ADAPTER.md))

  Add Java with setup-aware parsing, packages, symbols, test facts, and coverage
  through the same public rule-authoring model. Java support should expect
  classpath/build setup when deeper facts are requested, but rule authors should
  still consume normalized polint facts rather than raw javac, Maven, Gradle, or
  JaCoCo output.

## Release Gate For Each Item

Every roadmap item should ship with:

- [ ] public fact types or graph types
- [ ] public `RuleCtx` accessors
- [ ] adapter implementation for the targeted language tier
- [ ] cache-key participation when harvested facts change
- [ ] docs under `docs/facts/` when new facts are public
- [ ] external temp-repo test using only `polint::sdk::prelude::*`
- [ ] CLI `explain` or `graph` support when useful for debugging
- [ ] clear unsupported/setup diagnostics instead of silent empty facts

## Source Research

The deeper research and references live in
[`../CAPABILITY-FULFILLMENT-RESEARCH.md`](../CAPABILITY-FULFILLMENT-RESEARCH.md).
