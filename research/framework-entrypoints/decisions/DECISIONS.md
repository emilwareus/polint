# Decision Log

## D1. Boundary Facts Before Full Call Graph/Data Flow

Decision:

Implement framework/lifecycle entrypoint facts first. Feed them into call graph and data flow later through optional overlays.

Rationale:

- Entrypoints are useful by themselves.
- They are the root of reachability and taint paths.
- They exercise kernel provenance/validation/cache/merge with lower risk than sanitizer or global flow facts.

Rejected:

- Build global call graph first and hope routes appear as edges.
- Put framework routes directly into base call graph.

## D2. Native Rust Providers Are The Advanced Extension Surface

Decision:

Repo-local Rust providers should be the primary advanced extension mechanism. Declarative files can be activation manifests, fixtures, or generated data assets, but not the main capability surface.

Rationale:

- User explicitly wants users/agents to extend and alter analysis with Rust code.
- Rust providers can encode project-specific wrappers and algorithms better than static config.
- Typed sinks and validation keep engine invariants intact.

Rejected:

- Config-only framework models.
- Public mutable graph API.
- Dynamic library plugins.

## D3. Additive Merge First

Decision:

Extensions are additive first. They can add entrypoints, sources, sinks, summaries, and dispatch edges, but cannot suppress native exact facts initially.

Rationale:

- Additive entrypoints are safer than negative/suppression models.
- Bad sanitizer/barrier/suppression facts can create false negatives.
- Deterministic union with conflicts as diagnostics is explainable.

Rejected:

- Last-writer-wins.
- Extension deletion of native facts.

## D4. Unknowns Are Facts

Decision:

Dynamic route strings, unresolved handlers, missing setup, unsupported wrappers, budget exhaustion, and framework version gaps must emit facts/diagnostics.

Rationale:

- Absence is ambiguous.
- Unknown facts create agent integration tasks.
- Cache and evaluation need to know whether behavior was absent or unsupported.

Rejected:

- Silent missing facts.
- Placeholder fake facts.

## D5. First Implementation Scope Is Go And TS/JS

Decision:

Start with Go and TS/JS because they are current polint languages. Keep Python/Java/JVM as research input until adapters exist.

Rationale:

- Shipping facts for unsupported languages would create misleading public surface.
- Go and TS/JS already cover the first product need: routes, MCP tools, and repo-local framework wrappers.

Rejected:

- Blocking implementation on Python/Java adapters.
- Publicly advertising Java/Python entrypoint facts before adapters exist.

## D6. MCP Is A First-Class Boundary

Decision:

Model MCP tools/resources/prompts and protocol-level request handlers as first-class entrypoints and trust boundaries.

Rationale:

- polint's target user is AI agents.
- MCP servers expose privileged tools to agents.
- Request-side and return-side flows are both security-relevant.

Rejected:

- Treat MCP as just another function call or generic config file.

## D7. Borrow State Of The Art, Do Not Depend On It

Decision:

Use CodeQL, Pysa, FlowDroid, F4F, AutoWeb, Semgrep, CGMiner, and MCP-BiFlow as design references and benchmark sources. Do not embed them as core runtime dependencies.

Rationale:

- User wants full native implementation.
- External analyzers bring architecture, licensing, runtime, cache, and explainability constraints.
- polint needs a Rust-native extension and fact kernel.

Rejected:

- Shelling out to CodeQL/Pysa/Semgrep for core results.
