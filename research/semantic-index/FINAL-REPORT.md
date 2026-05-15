# Final Report: Semantic Index Research

Date: 2026-05-15

## Executive Decision

polint should implement semantic indexing as a set of **native Rust typed fact providers**, not as one generic graph database and not as a thin wrapper around external language servers.

The internal model should be:

```text
language adapter facts
  -> shared semantic fact contract
  -> explicit resolution/provenance/confidence side tables
  -> relation/fixpoint helpers for aliases/imports/reexports
  -> extension merge and validation
  -> typed SDK views
  -> optional SCIP/Kythe-like export
```

This matches the product goal: build a high-capability analysis engine that AI agents can extend with repo-local Rust code. The engine should expose uncertainty and extension hooks rather than pretend that a universal black-box resolver can infer every generated symbol, import trick, framework convention, and dynamic reference.

## What State Of The Art Actually Looks Like

State-of-the-art semantic indexing is not one algorithm. It is a family of architectures selected by language and product goal.

| Category | Best References | Lesson |
|---|---|---|
| Low-latency compiler/LSP semantics | rust-analyzer, TypeScript, gopls, Pyright, Ty/Pyrefly, JDT | Build language-native scopes, symbols, declarations, imports, type facts, and xrefs. Incremental architecture is part of correctness and UX. |
| Relational static analysis | CodeQL | Extract language facts into relations, derive recursive facts with QL, and use least-fixpoint semantics for name/data/call abstractions. Excellent for rules, expensive for always-on indexing. |
| JVM whole-program frameworks | Soot, SootUp, WALA | Java/JVM semantics need class loaders, classpaths, hierarchies, resolving levels, context-sensitive nodes, and explicit incomplete-world behavior. |
| Rule-oriented generic analysis | Semgrep | Generic AST plus naming is useful for ergonomic pattern rules but is not enough for high-accuracy semantic indexing. |
| Exchange/storage formats | SCIP, LSIF, Kythe | Use stable symbols, occurrences, relationships, VNames, and graph facts for export and code navigation. Do not make these the internal rule-time engine. |
| AI-oriented code indexes | AOCI, Semantic Code Graph work | Good reminder that agent users need symbolic-semantic retrieval, but current production-grade semantic accuracy still comes from compiler-style indexes. |

## Core Finding

The right abstraction is not:

```text
AST -> generic scope resolver -> universal symbol graph
```

The right abstraction is:

```text
Language-owned semantic providers
  emit normalized facts with stable IDs
  preserve language-specific precision and uncertainty
  attach provenance and validation
  support extension-provided facts
  feed a typed SDK and graph/export layers
```

The normalized layer must be strong enough for rules and cross-language algorithms, but it must not erase language semantics.

## Accuracy Lessons

### CodeQL

CodeQL is accurate where extractors and QL libraries model the language well, and powerful where recursive relational derivation is needed. It is not an IDE-style index. Database extraction and query evaluation are a heavier lifecycle. Precision is language-library-dependent, and query authors must understand which predicates are exact, approximate, or heuristic.

### rust-analyzer

rust-analyzer is the best architecture reference for a native Rust incremental semantic system. Its DefMap, HIR, item scopes, expression scopes, and semantic facade show how to separate syntax from semantic identity and avoid recomputing everything after small edits. Macros are the hard precision and invalidation problem.

### TypeScript

TypeScript's binder/checker model is the best reference for TS/JS semantics. It owns declaration merging, ambient modules, namespaces, imports, exports, control-flow narrowing, and type-driven lookup. It is accurate because it is the compiler. The lesson for polint is to build language-specific semantic providers, not to force TS into a generic resolver.

### gopls

gopls demonstrates a package-first architecture: metadata, parsed files, `go/types` info, import maps, method sets, and a serialized cross-reference index. The xref index is especially relevant: global references should not require scanning/rechecking the whole workspace every time.

### Pyright, Ty, Pyrefly

Python requires scopes, symbol flags, import semantics, flow-sensitive initialization, type narrowing, and explicit dynamic/unknown states. Ty and Pyrefly are especially valuable because they show modern Rust-native Python semantic designs. Pyright is the mature product reference; Ty/Pyrefly are the implementation-style references for polint.

### JDT, Soot, SootUp, WALA

Java/JVM semantic indexing cannot be reduced to file scopes. The semantic unit includes classpath/module path, class loader, packages, type hierarchy, bytecode/source bindings, resolving levels, and sometimes points-to/context. WALA's `method + context` call graph identity is a key warning for future call graph work.

### Semgrep

Semgrep shows that a generic AST and naming pass can support many useful rules, but it also shows the ceiling. Generic naming is not a substitute for compiler semantics.

### SCIP, LSIF, Kythe

SCIP is the cleanest modern exchange format. Kythe is the strongest durable graph identity model. LSIF is historically important but too coupled to LSP result graphs and too weak as an internal semantic model.

## Complexity Summary

| Phase | Typical Complexity | Main Risk |
|---|---:|---|
| Parse and syntax indexing | `O(source bytes)` | Parser recovery and stable spans. |
| Scope/declaration binding | `O(N + D)` | Language-specific namespaces and declaration merging. |
| Local reference lookup | `O(R * lexical_depth)`, usually near `O(R)` with scope maps | Shadowing, hoisting, globals/nonlocals, macro/generated scopes. |
| Import/export/alias fixpoint | `O(iterations * (I + aliases + exports))` | Cycles, star exports/imports, dynamic imports. |
| Type-assisted resolution | Language-specific; often near linear, sometimes superlinear | Generics, overloads, unions, narrowing, classpath/module size. |
| Global reference lookup | `O(candidate files + semantic verification)` | Name-only prefilters can miss generated/implicit refs if not modeled. |
| JVM class hierarchy/method resolution | `O(C + subtype edges + call sites * dispatch candidates)` for CHA/RTA tiers | Reflection, native code, invokedynamic, incomplete classpaths. |
| Export to SCIP/Kythe | `O(symbols + occurrences + relationships)` | Stable cross-run identity and schema compatibility. |

## Recommended Polint Design

### 1. Implement A Semantic Provider Stack

```text
polint.source
polint.<lang>.syntax
polint.<lang>.scopes
polint.<lang>.declarations
polint.<lang>.imports
polint.<lang>.references
polint.<lang>.resolution
polint.semantic.alias_fixpoint
polint.semantic.extension_merge
polint.semantic.xref_index
```

Each provider emits typed facts plus metadata. Metadata is not optional:

- stable key
- provider id/version
- input fact dependencies
- precision
- confidence
- validation status
- lifecycle/cache digest

### 2. Use Stable IDs At Three Levels

```text
RunId       fast arena ID, valid only inside one analysis run
StableKey   deterministic key for cache/evidence across runs
ExportKey   SCIP/Kythe-style cross-repo/corpus identity
```

Do not expose raw arena IDs through the public SDK.

### 3. Make Unknowns First-Class

Every unresolved, ambiguous, unsupported, generated, dynamic, or external target should be represented as a fact. This is critical for agent workflows:

```text
unknown generated symbol -> agent writes provider
ambiguous import alias -> agent adds repo-local model
dynamic framework reference -> framework provider emits synthetic symbol/reference
external package target -> module graph/provider records selected lifecycle boundary
```

### 4. Keep Language Semantics In Adapters

The shared semantic fact contract should normalize output, not force one lookup algorithm:

- Go: package/import/type object path semantics.
- TS/JS: binder/checker, declaration merging, namespaces, JS fallback.
- Python: scopes, symbols, imports, flow-sensitive initialization, type narrowing, dynamic states.
- Java/JVM: classpath, module/class loader, bindings, type hierarchy, bytecode/source identity.

### 5. Add A Relation/Fixpoint Helper, Not A Full Datalog Dependency

Use a small internal relation engine for:

- import/export closure;
- alias/reexport closure;
- generated symbol overlays;
- module graph closure;
- override/inheritance closure later.

Do not adopt CodeQL, Souffle, or a full Datalog runtime as the first engine dependency. The first native implementation should remain Rust-owned, typed, and easy to instrument.

### 6. Use Export Formats As Output Contracts

Support a future `polint semantic export --format scip` or Kythe-like export after internal facts are stable. Do not store rule-time facts internally as LSIF/SCIP/Kythe graphs.

## Implementation Priority

1. Deepen current `Symbols<'_>` and `References<'_>` facts with stable identity, roles, and resolution status.
2. Add internal `ScopeFact` and `ImportFact` providers for Go and TS/JS first.
3. Add `ResolutionFact` side tables and fact metadata.
4. Add alias/import/reexport fixpoint provider.
5. Add extension merge for generated symbols and reference resolution hints.
6. Add xref/name index for global reference queries.
7. Add optional SCIP export once fact keys are stable.
8. Research module graph next and feed its decisions back into import/package resolution.

## What To Avoid

- Do not depend on external language servers as the core engine. They are good references and temporary validation oracles, but they are not polint's native engine.
- Do not expose a public generic graph API before fact families are stable.
- Do not treat tree-sitter identifiers as semantic references without resolution status.
- Do not claim exactness for dynamic languages unless the provider can explain why.
- Do not collapse all names into one namespace; Go, Rust, TS, Python, and Java have different namespace rules.
- Do not make rule authors handle low-level AST nodes when a typed fact view is possible.

## Final Recommendation

Build the semantic index as the first serious vertical slice of the analysis kernel:

```text
Scopes + Imports + Symbols + References + ResolutionFacts
  with stable keys
  with provenance
  with extension-provided overlays
  with strict validation
```

This is the foundation that makes future module graph, framework entrypoints, call graphs, data flow, type/alias analysis, effects, and slicing coherent.
