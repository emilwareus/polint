# Abstract Interpretation Decision Log

## D1. Build A Domain Kernel, Not A Monolithic Value Analyzer

Decision: Use a reduced product of small domains.

Rationale: TypeScript/Pyright/rustc/Checker-style local domains deliver most
useful precision cheaply. APRON/ELINA and high-end abstract-interpretation
systems such as Eva/Astrée show that relational domains are powerful but costly.
A product kernel lets polint add precision incrementally.

## D2. Keep Raw Domain State Internal

Decision: Public rule authors get typed fact views, not raw lattice stores.

Rationale: Raw stores are unstable, difficult to document, and would freeze
implementation details. The SDK should expose queries and precision metadata.

## D3. Use Semantic Operations Instead Of Parser ASTs

Decision: Lower language adapters to semantic operations and places before
running domains.

Rationale: Go, TS/JS, Python, JVM, and future languages have different ASTs but
share many operations. ASTs remain evidence, not the abstract interpreter state.

## D4. Add Summaries Early

Decision: Every domain must have a summary projection before serious
interprocedural use.

Rationale: Without summaries, analysis stays local or becomes expensive
whole-program guessing. Summaries also provide the extension model boundary.

## D5. Make Extensions Law-Checked Products

Decision: Extensions emit typed products through sinks and cannot mutate kernel
state directly.

Rationale: Agent-authored Rust extensions are a product advantage, but only if
the kernel owns scheduling, merging, provenance, validation, cache keys, and
failure behavior.

## D6. Start With P0/P1 Domains

Decision: Implement reachability, nilness/nullish, truthiness, constants,
strings, initializedness, intervals, shapes, and typestate before relational
numeric precision.

Rationale: These domains are cross-language, high value for repo-local policy,
and much cheaper than packed octagons/polyhedra.

## D7. Relational Domains Are Selective

Decision: Treat octagons as the recommended first relational numeric candidate
for selected variable packs; avoid global polyhedra in the default engine.

Rationale: Octagons are the strongest first candidate for polint's relational
numeric tier, but `O(n^2)` memory and closure costs require bounded packs.
Polyhedra are expert-mode only.

## D8. Unknowns Are Facts

Decision: Emit explicit unknown/setup/budget/unsupported facts.

Rationale: polint's main user can be an AI agent that can turn unknowns into
repo-local models. Hidden false negatives waste that advantage.

## D9. Official Language Tools Are Allowed As Authority Inputs

Decision: Use official toolchain metadata where it is the semantic authority,
but do not depend on arbitrary OSS analyzers as runtime core.

Rationale: This matches prior roadmap guidance and keeps polint native while
respecting language compatibility.
