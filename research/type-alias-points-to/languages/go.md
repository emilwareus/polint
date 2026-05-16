# Go Report

## State Of The Art

The official Go analysis stack is the most important reference:

- `go/types` for type checking, aliases, method sets, interfaces, and generics;
- `go/packages` for package loading and build configuration;
- `golang.org/x/tools/go/ssa` for SSA IR;
- `golang.org/x/tools/go/callgraph/static`, `cha`, `rta`, and `vta` for call graph algorithms;
- analysis passes such as `buildssa`, `ctrlflow`, `nilness`, and staticcheck analyzers.

This research inspected the current `golang/tools` snapshot and found current call graph packages for static, CHA, RTA, and VTA. It did not find a current `go/pointer` package under `x/tools/go`; older Go Andersen-style pointer analysis should be treated as historical rather than a current primary implementation.

## Key Official Implementation Findings

### `go/types`

Use as the semantic oracle for:

- type identity;
- aliases and `Unalias`;
- method sets;
- interface satisfaction;
- pointer receiver versus value receiver behavior;
- generics/type parameters;
- selector resolution;
- package scopes and imports.

Polint native implication: Go type facts should match `go/types` semantics before claiming exactness.

### `go/ssa`

Use as an IR behavior oracle for:

- address-taken allocation values;
- `Alloc`, `Load`, `Store`, `FieldAddr`, `IndexAddr`, `MakeInterface`, `TypeAssert`, `ChangeInterface`, `MakeClosure`, `MakeMap`, `MakeSlice`, channels, functions, methods;
- closure/free variable behavior;
- calls and invokes;
- defer/go/select/panic-related behavior.

Polint native implication: even if polint does not depend on Go SSA at runtime, its native place/value/points-to facts should be validated against SSA-shaped fixtures.

### `x/tools/go/callgraph`

Inspected algorithms:

- static call graph;
- CHA;
- RTA;
- VTA.

The VTA implementation comments describe a type propagation graph where nodes carry types and function literals; it refines an initial call graph. Benchmark comments in `go/callgraph/callgraph_test.go` also note that the algorithms are unsound with respect to reflection.

Polint native implication: Go call graph tiers should consume type/value/points-to facts and expose reflection/unsafe unknowns.

## Go Accuracy Model

| Feature | Default polint target | Extension target |
|---|---|---|
| Packages/modules | Use existing Go lifecycle contract: module roots, `go.mod`, `go.work`, build tags, tests. | Repo-specific generated package roots or build conventions. |
| Types | Native facts validated against `go/types`. | Project generated code/model hints. |
| Method sets/interfaces | Implement enough to resolve direct/interface calls. | Reflection/proxy/framework dispatch. |
| Function values | Direct funcs, method values, closures. | Registries/callback frameworks. |
| Allocations | Composite literals, address-taken locals, `new`, `make`, closures. | Framework objects/singletons. |
| Points-to | Field-sensitive for structs and known maps/slices with conservative collapse. | Project-specific container/registry models. |
| Reflection/unsafe | Unknown/heuristic. | Validated explicit models only. |

## Complexity And Risk

Go is easier than Python/JS in many ways because the language has a strong type system and official toolchain, but several issues matter:

- monorepos and multiple module roots;
- build tags and test variants;
- generics and aliases;
- reflection;
- unsafe;
- interface dispatch;
- method values and closures;
- map/slice/channel element precision;
- generated code.

## Recommended Go Implementation Path

```text
1. Reuse existing Go lifecycle root-selection model.
2. Build native package/type/member/method-set facts.
3. Emit Go places for locals, params, receivers, globals, fields, indexes.
4. Emit allocation tokens for address-taken locals, composite literals, new/make, closures.
5. Emit function/method object value facts.
6. Implement static and CHA-like call facts.
7. Implement RTA-like reachable allocation/type refinement.
8. Implement VTA-like type/function propagation using the shared value-flow graph.
9. Add bounded Andersen constraints for alias-heavy rules.
10. Validate against `go/types`, `go/ssa`, and `x/tools/go/callgraph` fixtures.
```

## Native Versus Oracle

For the "full native implementation" goal, do not call Go tools as the polint runtime analyzer. Use them as:

- fixture generators;
- differential validation oracles;
- behavior references;
- compatibility checks during development.

The runtime engine should emit polint-owned facts with polint-owned IDs, provenance, precision labels, and cache keys.
