# Requirements: polint v1.4 Policy Query Surface

**Defined:** 2026-06-20
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Milestone goal:** Promote the useful parts of the private graph, control-flow, and data-flow engine into a small preview SDK for policy queries. Rule authors should express advanced repo-local policies through typed query objects and pattern structs, not raw graph traversal or ad hoc DSLs.

**Source context:** v1.2 built private CFG, direct/refined calls, data-flow, path evidence, summaries, demand queries, and promotion gates. v1.3 improved the shared semantic graph, reachability roots, Go/TS call solving, solver budgets, unknown taxonomy, adaptation models, and benchmark gates. v1.4 turns the validated substrate into a user-facing rule-authoring surface while keeping solver internals private.

## API Design Contract

v1.4 should have one obvious way to write these rules:

```rust
#[polint::rule(
    id = "local/no-secret-logs",
    description = "Secret-like values must not flow to logs without redaction.",
    severity = "error"
)]
pub(crate) fn no_secret_logs(ctx: &mut RuleCtx<'_>, flow: DataFlow<'_>) -> RuleResult {
    let mut query = FlowQuery::new(
        SourcePattern::secret_like(["token", "password", "apiKey"]),
        SinkPattern::logger(),
    );
    query.barriers = BarrierPattern::call_any(["redact", "mask_secret"]);
    query.max_paths = 20;

    for violation in flow.forbidden(query) {
        ctx.report(violation.diagnostic(
            ctx.rule_id(),
            "Secret-like value reaches logging without redaction.",
        ));
    }

    Ok(())
}
```

The public style is:

1. Request a typed preview view in the `#[polint::rule]` function signature.
2. Construct one plain query object with typed patterns.
3. Run one query method on the view.
4. Report each returned violation through its diagnostic helper.

Do not add competing public forms for the same behavior: no fluent builder DSL, no closure-filter DSL, no string query language, no public raw CFG/call/data-flow graph traversal as the normal rule-authoring path.

## v1 Requirements

Requirements for the v1.4 milestone. Each maps to exactly one roadmap phase.

### Preview SDK Surface

- [x] **API-01**: Rule authors can request preview fact views `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`, and `DataFlow<'_>` from `#[polint::rule]` signatures using only `polint::sdk::prelude::*`.
- [x] **API-02**: Advanced policy rules use one public query-object style: `Query::new(required...)`, explicit option fields, a single view method, and violation results. Fluent DSLs, closure filters, string mini-languages, and raw graph traversal are not supported public APIs.
- [x] **API-03**: Query structs exist for `ReachQuery`, `GuardQuery`, `LifecycleQuery`, and `FlowQuery`, with stable required inputs, explicit defaults, deterministic ordering, and documented budget knobs.
- [x] **API-04**: Pattern structs exist for `EventPattern`, `SourcePattern`, `SinkPattern`, `GuardPattern`, and `BarrierPattern`, covering calls, imports/packages, fields/properties, HTTP/trust boundaries, secrets, PII-like values, loggers, command execution, network requests, HTML sinks, and persistence writes.
- [x] **API-05**: Macro-derived capabilities, rule manifests, and capability diagnostics understand the new preview views; unsupported setup produces `polint/capability` diagnostics and the rule does not run with placeholder facts.
- [x] **API-06**: Existing reserved low-level `Cfg<'_>` and `CallGraph<'_>` remain reserved; v1.4 promotes policy-level `ControlFlow<'_>` and `Calls<'_>` instead of exposing raw CFG or call-graph internals.

### Events and Calls

- [x] **CALL-01**: `Events<'_>` can match semantic call events through `EventPattern::call` without exposing raw AST, MIR, CFG, solver, or graph node IDs. Non-call event families remain preview vocabulary until later phases promote backed facts.
- [x] **CALL-02**: `Calls<'_>` supports `forbidden_reachable(ReachQuery)` for policies such as "no raw API reachable from request handlers" and returns deterministic violations with root, callsite, target, precision, and unknown/budget evidence.
- [x] **CALL-03**: Reach queries can constrain roots, target patterns, tests inclusion, max depth, max paths, minimum precision, and minimum confidence. Package/module scoping remains intentionally deferred instead of adding a second public query style.
- [x] **CALL-04**: Call-query behavior is backed by the v1.3 refined-call projection and unknown taxonomy, preserving precision floors and surfacing unresolved or budget-exceeded edges honestly.

### Control-Flow Policies

- [x] **CTRL-01**: `ControlFlow<'_>` supports same-function call-event `missing_guard(GuardQuery)` for policies such as validation-before-money-move and allowlist-before-dangerous-call. Field/property write events remain preview vocabulary until backed write-event facts land.
- [x] **CTRL-02**: `ControlFlow<'_>` supports same-function call-event `missing_cleanup(LifecycleQuery)` for policies such as transaction begin followed by rollback/cleanup. Exact resource identity and every-exit cleanup proof remain deferred.
- [x] **CTRL-03**: Guard and lifecycle queries expose one typed public API for same-function checks without exposing dominance/postdominance graphs directly. Bounded interprocedural execution remains deferred behind the existing `max_depth` shape.
- [x] **CTRL-04**: Control-flow results include event spans, guard/cleanup candidates, same-function uncovered path evidence, conservative status/precision, and budget status in diagnostic evidence.

### Data-Flow Policies

- [x] **FLOW-01**: `DataFlow<'_>` supports `forbidden(FlowQuery)` for backed source-to-sink policies with optional barriers/sanitizers through `BarrierPattern::call_any`.
- [x] **FLOW-02**: `DataFlow<'_>` supports required-barrier semantics for call-based policies, including request-to-dangerous-call and secret-to-log patterns, by suppressing paths that cross a matching sanitizer/barrier call and reporting uncovered paths.
- [x] **FLOW-03**: Built-in Phase 58 patterns cover HTTP request trust-boundary sources, explicit `secret_like` name sources, exact call sinks, logger sinks, and explicit barrier calls. Broader built-in categories such as SQL, raw HTML/JSX, SSRF URLs, file paths, analytics, PII, and outbound network clients remain template/future taxonomy work.
- [x] **FLOW-04**: Flow queries use the existing private data-flow substrate for bounded path search, source-introduction edges, existing local/direct-call/summary edges, deterministic capped results, and path evidence without exposing raw graph APIs.
- [x] **FLOW-05**: Data-flow queries distinguish found, heuristic, unknown, and budget-exceeded results in policy diagnostics; unsupported pattern families return no matches until backed, and heuristic patterns are documented honestly.

### Violations, Evidence, Cache, and Unknowns

- [x] **EVID-01**: All query families return a consistent `PolicyViolation`-style result that can produce a diagnostic with rule ID, message, primary span, labels, suggestions when available, and structured evidence.
- [x] **EVID-02**: Violation evidence includes query type, matched patterns, root/source/sink/event spans, path steps, precision/confidence/status, budget state, and unknown reasons in stable JSON/SARIF output.
- [x] **EVID-03**: Results are deterministically sorted and deduplicated across sequential/parallel execution, cache restore, provider ordering, and repeated runs.
- [x] **EVID-04**: Query parameters, preview API versions, rule options, language lifecycle inputs, solver budgets, and model/adaptation files participate in cache identity with must-invalidate and must-preserve-hit tests.
- [x] **EVID-05**: Unknown and budget behavior is user-visible and actionable; policy rules must not silently pass when setup gaps, unsupported semantics, or budget exhaustion make the answer incomplete.

### Flagship Rule Templates

- [x] **TPL-01**: `polint new-rule` can generate a request-to-shell template using `DataFlow<'_>`, `FlowQuery`, `SourcePattern::http_request`, `SinkPattern::call`, and validation barriers.
- [x] **TPL-02**: Generated templates cover secret-to-log and PII-to-analytics policies with explicit heuristic wording and redaction/barrier examples.
- [x] **TPL-03**: Generated templates cover auth/validation-before-sensitive-write, transaction cleanup, and raw reachable API policies using `ControlFlow<'_>` and `Calls<'_>`.
- [x] **TPL-04**: Generated templates cover SSRF, dangerous HTML sinks, unsafe deserialization from request data, and user-controlled file path policies using the same query-object style.
- [x] **TPL-05**: README, examples, generated agent skill text, and docs show the flagship templates as repo-local policy examples without presenting polint as a bundled ruleset.

### Validation and Public Boundary

- [ ] **VAL-01**: Each preview view and each query family has at least one temp-repo style test where generated `.polint/rules` imports only `polint::sdk::prelude::*`, registers through `polint::runner::run_cli`, consumes real facts, and asserts diagnostics through `polint check --format json`.
- [ ] **VAL-02**: Public docs under `docs/facts/` describe the preview status, syntax, limits, precision tiers, heuristic behavior, unknown/budget semantics, and realistic examples for every new view and query type.
- [ ] **VAL-03**: The public-surface leak gate proves raw CFG, call graph, semantic graph, data-flow graph, solver, provider, `AnalysisDb`, and private IDs are not reachable from the supported SDK, CLI, runner, README, generated skill text, or docs/facts surfaces.
- [ ] **VAL-04**: Milestone exit verification runs full workspace tests, formatting, clippy, temp-repo SDK tests, cache invalidation tests, docs/example smoke tests, and deterministic repeated-run checks for the flagship policies.

## Future Requirements

Deferred to v1.5+ unless explicitly pulled forward.

### Stable Query Surface

- **STABLE-FUT-01**: Promote preview query APIs to stable after at least one milestone of external-rule usage and compatibility review.
- **STABLE-FUT-02**: Add semver-stable JSON schema for query evidence consumed by agents and IDEs.
- **STABLE-FUT-03**: Add migration tooling if preview query names or fields change before stabilization.

### More Precision

- **PREC-FUT-01**: Context sensitivity controls for specific flow/call queries.
- **PREC-FUT-02**: Opt-in bounded Andersen/VTA precision exposed as query options after benchmark gates.
- **PREC-FUT-03**: Language-specific framework packs for Rails/Django/Spring/etc. after Go and TS/JS policy queries prove the model.

### Interactive Querying

- **QUERY-FUT-01**: Public `polint query` command for exploratory policy queries outside Rust rule code.
- **QUERY-FUT-02**: IDE/LSP integration that visualizes policy paths and unknowns.
- **QUERY-FUT-03**: Agent-editable policy templates with guarded auto-fix suggestions.

## Out of Scope

Explicitly excluded from v1.4 to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Raw public CFG, call graph, semantic graph, solver, or data-flow graph APIs | The product value is simple policy authoring. Raw graph APIs freeze internals and create many ways to do the same thing. |
| A string query language | Adds parser, documentation, escaping, and partial-overlap problems with Rust query structs. Rust rules already give users a typed host language. |
| Fluent builder DSLs for every query option | Creates multiple equivalent spellings and fights the "one good way" API goal. Use `Query::new(...)` plus explicit option fields. |
| Bundled production ruleset | polint remains a framework for repo-local rules. Templates are examples/scaffolds, not shipped default policy. |
| Perfect whole-program precision | Preview APIs must expose precision/unknown/budget limits honestly rather than pretending exact coverage. |
| Auto-fixing advanced policy violations | Most violations require domain judgment. v1.4 can include suggestions/evidence, not automatic rewrites. |
| Python/Java parity | Go and TS/JS remain the proving languages for this policy query surface. |
| Public adaptation/model-pack authoring surface | Existing adaptation internals may feed query answers, but v1.4 should not expose a separate model-pack SDK. |
| Replacing ESLint, Biome, Ruff, golangci-lint, or formatters | These policy queries target repo-specific semantic policies generic linters cannot know. |

## Traceability

Which phases cover which requirements.

| Requirement | Phase | Status |
|-------------|-------|--------|
| API-01 | Phase 55 | Complete |
| API-02 | Phase 55 | Complete |
| API-03 | Phase 55 | Complete |
| API-04 | Phase 55 | Complete |
| API-05 | Phase 55 | Complete |
| API-06 | Phase 55 | Complete |
| CALL-01 | Phase 56 | Complete |
| CALL-02 | Phase 56 | Complete |
| CALL-03 | Phase 56 | Complete |
| CALL-04 | Phase 56 | Complete |
| CTRL-01 | Phase 57 | Complete |
| CTRL-02 | Phase 57 | Complete |
| CTRL-03 | Phase 57 | Complete |
| CTRL-04 | Phase 57 | Complete |
| FLOW-01 | Phase 58 | Complete |
| FLOW-02 | Phase 58 | Complete |
| FLOW-03 | Phase 58 | Complete |
| FLOW-04 | Phase 58 | Complete |
| FLOW-05 | Phase 58 | Complete |
| EVID-01 | Phase 59 | Complete |
| EVID-02 | Phase 59 | Complete |
| EVID-03 | Phase 59 | Complete |
| EVID-04 | Phase 59 | Complete |
| EVID-05 | Phase 59 | Complete |
| TPL-01 | Phase 60 | Complete |
| TPL-02 | Phase 60 | Complete |
| TPL-03 | Phase 60 | Complete |
| TPL-04 | Phase 60 | Complete |
| TPL-05 | Phase 60 | Complete |
| VAL-01 | Phase 61 | Planned |
| VAL-02 | Phase 61 | Planned |
| VAL-03 | Phase 62 | Planned |
| VAL-04 | Phase 62 | Planned |

**Coverage:**
- v1.4 requirements: 33 total
- Mapped to phases: 33 (100%)
- Unmapped: 0

**Phase coverage breakdown:**
- Phase 55: API-01, API-02, API-03, API-04, API-05, API-06 (6 reqs)
- Phase 56: CALL-01, CALL-02, CALL-03, CALL-04 (4 reqs)
- Phase 57: CTRL-01, CTRL-02, CTRL-03, CTRL-04 (4 reqs)
- Phase 58: FLOW-01, FLOW-02, FLOW-03, FLOW-04, FLOW-05 (5 reqs)
- Phase 59: EVID-01, EVID-02, EVID-03, EVID-04, EVID-05 (5 reqs)
- Phase 60: TPL-01, TPL-02, TPL-03, TPL-04, TPL-05 (5 reqs)
- Phase 61: VAL-01, VAL-02 (2 reqs)
- Phase 62: VAL-03, VAL-04 (2 reqs)

---
*Requirements defined: 2026-06-20*
