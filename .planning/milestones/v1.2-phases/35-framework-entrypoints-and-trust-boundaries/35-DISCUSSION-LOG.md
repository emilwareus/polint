# Phase 35: Framework Entrypoints and Trust Boundaries - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-23
**Phase:** 35-framework-entrypoints-and-trust-boundaries
**Areas discussed:** Fact family scope, Default recognizer tier, Provider placement, Extension overlay integration, Trust boundary representation
**Mode:** `--auto` (all areas auto-selected, recommended options chosen)

---

## Fact Family Scope and Design

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal first-tier set | EntrypointFact, TrustBoundaryFact, FrameworkDispatchEdgeFact, UnresolvedFrameworkFact only | ✓ |
| Full research set | All 9 fact families from research (including Component, Registration, Lifecycle, Source, SinkBoundary) | |
| Entrypoints only | Just EntrypointFact and UnresolvedFrameworkFact, defer trust boundaries | |

**User's choice:** [auto] Minimal first-tier set (recommended default)
**Notes:** Four families are sufficient to prove the framework boundary layer end-to-end. Richer component/lifecycle modeling is deferred.

---

## Default Recognizer Tier

| Option | Description | Selected |
|--------|-------------|----------|
| Scoped first tier | Go: net/http, chi. TS/JS: Express, MCP SDK. Plus test/CLI for both. | ✓ |
| Broad first tier | Add gin, echo, gorilla/mux, Fastify, Nest decorators, Koa | |
| Minimal proof | Only net/http and Express, everything else deferred | |

**User's choice:** [auto] Scoped first tier (recommended default)
**Notes:** net/http + chi cover most Go HTTP apps. Express + MCP SDK cover the most common TS/JS patterns and the AI-agent-era differentiator. Test and CLI recognizers are cheap Tier 0 wins.

---

## Provider Placement and Architecture

| Option | Description | Selected |
|--------|-------------|----------|
| Single polint.entrypoints provider | After polint.calls, before polint.extensions. Language-specific extraction behind shared output. | ✓ |
| Per-language providers | polint.go.entrypoints and polint.ts.entrypoints as separate providers | |
| Extension-only approach | No native provider; all framework facts come from extensions | |

**User's choice:** [auto] Single polint.entrypoints provider (recommended default)
**Notes:** Follows the established pattern of shared providers with language-specific extraction behind the boundary. One provider simplifies cache identity and eval observation.

---

## Extension Overlay Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Through Phase 34 typed sinks | Extensions emit entrypoint/trust_boundary facts through existing ExtensionFactCandidate | ✓ |
| Dedicated extension entrypoint sink | New EntrypointSink type for extensions beyond generic ExtensionFactCandidate | |
| No extension integration in Phase 35 | Defer extension framework facts entirely to Phase 41 | |

**User's choice:** [auto] Through Phase 34 typed sinks (recommended default)
**Notes:** The Phase 34 extension infrastructure already handles discovery, validation, merge, and eval. Framework facts flow through naturally as new fact families.

---

## Trust Boundary Representation

| Option | Description | Selected |
|--------|-------------|----------|
| Separate TrustBoundaryFact family | Per-entrypoint, per-source-kind facts with typed source kinds | ✓ |
| Embedded in EntrypointFact | Trust boundary data as nested fields on entrypoint facts | |
| Deferred to Phase 38 | Only entrypoints in Phase 35; trust boundaries wait for data flow | |

**User's choice:** [auto] Separate TrustBoundaryFact family (recommended default)
**Notes:** Separate facts allow per-source-kind precision, independent validation, and clean consumption by Phase 38 data flow as taint sources.

---

## Claude's Discretion

- Module layout and naming within the crate-private boundary
- Whether recognizers are separate files or methods on a shared type
- Exact FactFamily enum variant naming
- Whether dispatch edges are same-pass or post-pass
- Whether to add ProviderKind::FrameworkAnalysis or reuse WholeRepoDerived
- Plan split across contracts/provider/recognizers/trust-boundaries/extension/eval

## Deferred Ideas

- Fastify, Nest, Koa, Hapi, gin, echo, gorilla/mux recognizers: future phase or extension overlay
- Middleware ordering and lifecycle composition: later phases
- FrameworkComponentFact, RegistrationFact, LifecycleFact: richer modeling phases
- Public Entrypoints<'_> SDK view: Phase 41
- Data-flow source/sink wiring from trust boundaries: Phase 38
