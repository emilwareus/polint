# TypeScript / JavaScript Report

## State Of The Art

TS/JS analysis is split across several traditions:

- TypeScript compiler: industrial structural type checking and flow narrowing.
- Oxc: Rust-native parser, semantic scopes, symbols, references, and growing CFG infrastructure.
- Flow: type checker with refinement/incrementality lessons.
- TAJS: whole-program abstract interpretation for JavaScript.
- Jelly: modern JS/TS call graph and points-to style analysis.
- CodeQL JS: query-facing type tracking, API modeling, and data-flow libraries.

No single implementation solves the whole problem. polint should combine their lessons as native fact layers.

## TypeScript Compiler

The TypeScript compiler creates flow nodes in the binder and computes narrowed types lazily in the checker.

Key inspected paths:

- `typescript/src/compiler/binder.ts`
- `typescript/src/compiler/checker.ts`
- `typescript/src/compiler/types.ts`

Important source findings:

- `FlowFlags` and `FlowNode` are central to narrowing.
- `createFlowNode`, `createFlowCondition`, `createFlowMutation`, and `createFlowCall` are binder helpers.
- `checker.ts` has `getTypeAtFlowNode`, `getTypeAtFlowCondition`, `getTypeAtFlowCall`, `narrowTypeByEquality`, `narrowTypeByInstanceof`, `narrowTypeByInKeyword`, and discriminant narrowing logic.
- The compiler uses caching and flow-depth controls.

Polint lesson: implement reference/location-specific narrowed type facts. Do not treat TypeScript as a points-to engine.

## Oxc

Oxc is the best Rust-native baseline for polint's existing TS/JS adapter. It gives parser, AST, semantic scopes, symbols, references, and resolver infrastructure. It also has CFG work.

Key inspected paths:

- `oxc/crates/oxc_semantic`
- `oxc/crates/oxc_cfg`
- `oxc/crates/oxc_resolver`
- `oxc/crates/oxc_linter`

Polint lesson: use Oxc as a parser/semantic input where polint already depends on it, but keep polint-owned fact IDs, precision labels, and analysis provider boundaries.

## Flow

Flow is relevant for refinements, type inference, and incremental analysis. It is less directly usable for Rust implementation, but its architecture validates the idea that JS precision lives in a type/refinement engine first.

Polint lesson: local refinements and invalidation are core; exact heap aliasing is secondary.

## TAJS

TAJS is a mature academic/OSS JavaScript abstract interpreter. It models JS heap/value semantics more deeply than normal linters or type checkers.

Key inspected paths:

- `tajs/src/dk/brics/tajs/analysis`
- `tajs/src/dk/brics/tajs/flowgraph`
- `tajs/src/dk/brics/tajs/lattice`
- `tajs/src/dk/brics/tajs/js2flowgraph`

Polint lesson: abstract domains and heap objects are powerful but costly. Use this as inspiration for optional precision tiers, not as the default rule engine.

## Jelly

Jelly is a modern JS/TS analysis from the Aarhus group. It is directly relevant for call graph and points-to style analysis with pragmatic JavaScript/TypeScript handling.

Polint lesson: property/access-path flow and function-object propagation are the right middle ground between TypeScript narrowing and full abstract interpretation.

## CodeQL JS

CodeQL JS is a mature query-facing system. Its type tracking and API modeling are more relevant to polint's rule authoring than a raw pointer graph.

Official docs:

- CodeQL JavaScript type tracking for API modeling.
- CodeQL JS data-flow and library docs.

Polint lesson: expose high-level typed query views and model hooks. Rule authors should not have to manipulate solver internals.

## TS/JS Accuracy Model

| Feature | Default polint target | Extension target |
|---|---|---|
| Lexical bindings | Exact from Oxc semantic facts. | Generated globals/modules. |
| Imports/exports | Native module graph plus resolver facts. | Bundler/plugin/framework resolution. |
| Declared types | Parse common TS annotations and declarations. | Repo-specific generated types. |
| Narrowing | `typeof`, equality, truthiness, `instanceof`, `in`, discriminants, optional chain nullish facts. | Custom type predicates/assertions/validators. |
| Function objects | Direct functions, arrows, methods, imports, assignments. | Registries, React hooks, routers, dependency injection. |
| Object properties | Object literals, classes, known keys, string literal keys. | Dynamic framework properties and generated models. |
| Points-to | Function/object/module tokens, property-sensitive where known. | Framework-specific object identity and callback wiring. |
| Dynamic behavior | `eval`, unknown computed keys, proxies, dynamic imports as unknown. | Validated repo-specific models. |

## Complexity And Risk

TS/JS complexity comes from:

- structural types and large unions;
- overloaded/conditional/generic types;
- computed property keys;
- prototype mutation;
- `this` binding;
- closures;
- async/promises;
- bundler/module-resolution behavior;
- framework callback registration.

Default mode should not try to model all runtime JS semantics. Instead:

- use high-confidence type/value/property facts;
- emit unknowns for dynamic constructs;
- let agent extensions add framework models.

## Recommended TS/JS Implementation Path

```text
1. Oxc semantic symbol/reference/import facts
2. places/access paths for locals, imports, exports, properties, this
3. value facts for functions, classes, modules, object literals, literals
4. local narrowed type facts from CFG conditions
5. property-shape facts for object/class literals
6. callback registration summaries
7. bounded points-to for function/object/property tokens
8. extension sinks for framework routing, dependency injection, generated APIs
```

This supports fuller call graphs and stronger data-flow without requiring a complete JavaScript abstract interpreter as the first implementation.
