# Semantic Index Decision Log

## D1. Do Not Build One Generic Semantic Index

Decision: use language-owned providers that emit normalized facts.

Rationale: TypeScript, gopls, Pyright, Ty, JDT, and rust-analyzer get accuracy from language-specific semantic rules. Semgrep shows the useful but limited ceiling of generic naming.

Rejected alternative: one tree-sitter/generic-AST resolver for all languages.

## D2. Do Not Depend On External Language Servers For The Core Engine

Decision: external tools are validation oracles and design references, not runtime dependencies.

Rationale: the product goal is a native Rust engine with controlled cache keys, provenance, extension merges, and rule SDK behavior.

Rejected alternative: shell out to gopls/tsserver/pyright/JDT for semantic facts.

## D3. Add Resolution Status As A First-Class Fact

Decision: every reference/import has a resolution status.

Rationale: exact, ambiguous, unresolved, dynamic, external, generated, extension-asserted, and unsupported states are materially different for rules and agents.

Rejected alternative: `Option<SymbolId>` only.

## D4. Use A Small Internal Relation/Fixpoint Helper

Decision: add a typed internal helper for recursive alias/import/export relations.

Rationale: CodeQL demonstrates the value of relational fixpoints, but adopting a full Datalog/CodeQL/Souffle runtime first would slow iteration and complicate Rust-native fact metadata.

Rejected alternative: full Datalog engine in v1 semantic index.

## D5. Use SCIP/Kythe For Export Concepts, Not Internal Storage

Decision: internal facts remain typed Rust arenas and indexes; export adapters can emit SCIP/Kythe-like shapes later.

Rationale: SCIP and Kythe are excellent interchange/storage schemas, but rule-time queries need typed, demand-shaped indexes and metadata.

Rejected alternative: internal LSIF/SCIP/Kythe graph as primary store.

## D6. Extension Facts Must Be Validated And Provenanced

Decision: repo-local Rust providers can add semantic facts, but every fact carries extension provenance and validation status.

Rationale: agent-authored extensions are the product differentiator, but unvalidated extensions must not silently override native exact facts.

Rejected alternative: extension facts merge as if native.

## D7. Research Module Graph Next

Decision: module/package/repo topology research should follow this track before deep call graph/data-flow implementation.

Rationale: stable symbol keys, import resolution, external symbols, generated-code zones, package roots, and lifecycle config all depend on module graph semantics.
