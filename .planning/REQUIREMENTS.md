# Requirements: polint Capability Fulfillment

**Defined:** 2026-05-08
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

## v1.1 Requirements

Requirements for the Capability Fulfillment milestone. Each requirement maps to
one roadmap phase.

### Capability Planning

- [x] **PLAN-01**: Rule authors can declare capabilities and see an explicit analysis plan derived from enabled rules.
- [ ] **PLAN-02**: The runner passes the resolved analysis plan to Go and TS/JS adapters before fact harvesting.
- [ ] **PLAN-03**: Cache keys change when requested capabilities or setup-sensitive analysis inputs change.
- [x] **PLAN-04**: Missing or unsupported setup for requested capabilities becomes a clear diagnostic or structured warning.

### Control Flow Graphs

- [ ] **CFG-01**: Rule authors can read real per-function CFG facts through `RuleCtx`.
- [ ] **CFG-02**: Go functions expose syntax-level CFGs for branches, loops, switches, returns, and exits.
- [ ] **CFG-03**: TS/JS functions expose syntax-level CFGs through the shared graph model.
- [ ] **CFG-04**: `polint graph cfg` renders non-placeholder CFG output for debugging.

### Coverage Facts

- [ ] **COV-01**: Rule authors can read coverage facts for files, functions, and branches through `RuleCtx`.
- [ ] **COV-02**: Go `coverprofile` input maps to repo-relative coverage facts.
- [ ] **COV-03**: TS/JS LCOV input maps to repo-relative coverage facts.
- [ ] **COV-04**: Coverage facts expose precision/source metadata and report missing setup clearly.

### Module Resolution

- [ ] **MOD-01**: Rule authors can read resolved import facts and unresolved import reasons through `RuleCtx`.
- [ ] **MOD-02**: TS/JS imports resolve through project-aware resolver setup such as `tsconfig` and package metadata.
- [ ] **MOD-03**: Go imports resolve through Go package/module information where setup is available.
- [ ] **MOD-04**: A module graph exposes file, package, module, and dependency relationships for architecture rules.

### Call Graph

- [ ] **CALL-01**: Rule authors can read direct call edge facts through `RuleCtx`.
- [ ] **CALL-02**: Go and TS/JS call facts include caller, callee text, span, resolution status, and confidence.
- [ ] **CALL-03**: Call graph facts consume resolved imports and symbols when available.
- [ ] **CALL-04**: `polint graph calls` renders useful call graph output for debugging.

### Symbols And References

- [ ] **SYM-01**: Rule authors can read symbol, definition, and reference facts through `RuleCtx`.
- [ ] **SYM-02**: Go symbols and references are populated from typed package information where setup is available.
- [ ] **SYM-03**: TS/JS symbols and references are populated from Oxc semantic facts where setup is available.
- [ ] **SYM-04**: Symbol/reference facts expose precision tiers and stable IDs suitable for diagnostics and cache restore.

### Test Metrics

- [ ] **TEST-01**: Rule authors can read normalized test-suite metrics through `RuleCtx`.
- [ ] **TEST-02**: Go metrics aggregate existing test facts into assertions, subtests, table rows, evidence terms, and related test evidence.
- [ ] **TEST-03**: TS/JS metrics detect common Jest/Vitest/Mocha-style test structures and assertion evidence.
- [ ] **TEST-04**: Test metrics state heuristic limits and avoid claiming exact behavioral coverage.

### Python Adapter

- [ ] **PY-01**: Python files participate in discovery, parsing, diagnostics, and the shared fact model.
- [ ] **PY-02**: Python adapter exposes the declared initial capability tier: syntax, functions/classes, imports, literals, branches, tests, and coverage import.
- [ ] **PY-03**: Python import/call uncertainty and optional interpreter or virtualenv setup are represented explicitly.
- [ ] **PY-04**: Python rule packs can be written against `polint::sdk::prelude::*` with external-consumer tests.

### Java Adapter

- [ ] **JAVA-01**: Java files participate in discovery, parsing, diagnostics, and the shared fact model.
- [ ] **JAVA-02**: Java adapter exposes the declared initial capability tier: packages/imports, classes/methods, literals, branches, tests, and coverage import.
- [ ] **JAVA-03**: Java classpath/build setup requirements are represented explicitly when deeper facts are requested.
- [ ] **JAVA-04**: Java rule packs can be written against `polint::sdk::prelude::*` with external-consumer tests.

## Future Requirements

Deferred until after Go and TS/JS prove the full capability model:

- **DATA-01**: Rule authors can consume cross-language dataflow facts.
- **TAINT-01**: Rule authors can define and consume source/sink taint facts.
- **TYPE-01**: Rule authors can consume deeper type-aware facts beyond symbol/reference resolution.
- **LANG-01**: Additional languages can be added through the adapter contract after Python and Java.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Perfect semantic precision in the first implementation of every capability | The milestone should expose precision tiers and useful facts incrementally instead of pretending all dynamic or setup-sensitive behavior is exact. |
| Running user test suites inside polint by default | Coverage should be imported from reports that users or CI already produce. |
| Exposing raw language-tool output as the public SDK | Rule authors should consume normalized polint facts, not raw `go/packages`, Oxc, Python, javac, Maven, Gradle, or coverage report structures. |
| Python and Java parity before Go and TS/JS capability coverage | Go and TS/JS should prove the complete model first; Python and Java start with declared subsets and expand later. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PLAN-01 | Phase 11 | Complete |
| PLAN-02 | Phase 11 | Pending |
| PLAN-03 | Phase 11 | Pending |
| PLAN-04 | Phase 11 | Complete |
| CFG-01 | Phase 12 | Pending |
| CFG-02 | Phase 12 | Pending |
| CFG-03 | Phase 12 | Pending |
| CFG-04 | Phase 12 | Pending |
| COV-01 | Phase 13 | Pending |
| COV-02 | Phase 13 | Pending |
| COV-03 | Phase 13 | Pending |
| COV-04 | Phase 13 | Pending |
| MOD-01 | Phase 14 | Pending |
| MOD-02 | Phase 14 | Pending |
| MOD-03 | Phase 14 | Pending |
| MOD-04 | Phase 14 | Pending |
| CALL-01 | Phase 15 | Pending |
| CALL-02 | Phase 15 | Pending |
| CALL-03 | Phase 15 | Pending |
| CALL-04 | Phase 15 | Pending |
| SYM-01 | Phase 16 | Pending |
| SYM-02 | Phase 16 | Pending |
| SYM-03 | Phase 16 | Pending |
| SYM-04 | Phase 16 | Pending |
| TEST-01 | Phase 17 | Pending |
| TEST-02 | Phase 17 | Pending |
| TEST-03 | Phase 17 | Pending |
| TEST-04 | Phase 17 | Pending |
| PY-01 | Phase 18 | Pending |
| PY-02 | Phase 18 | Pending |
| PY-03 | Phase 18 | Pending |
| PY-04 | Phase 18 | Pending |
| JAVA-01 | Phase 19 | Pending |
| JAVA-02 | Phase 19 | Pending |
| JAVA-03 | Phase 19 | Pending |
| JAVA-04 | Phase 19 | Pending |

**Coverage:**
- v1.1 requirements: 36 total
- Mapped to phases: 36
- Unmapped: 0

---
*Requirements defined: 2026-05-08*
*Last updated: 2026-05-08 after starting milestone v1.1 Capability Fulfillment*
