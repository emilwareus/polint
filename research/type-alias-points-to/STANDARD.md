# Standard Research Structure

This document defines the comparison structure used across this research track.

## Terms

| Term | Meaning In This Research |
|---|---|
| Type analysis | Computes declared, inferred, narrowed, structural, nominal, or runtime type sets for expressions, symbols, fields, and call targets. |
| Value analysis | Computes abstract values such as constants, truthiness, nullness, object/function tokens, enum/literal values, string sets, and coarse numeric facts. |
| Place analysis | Represents assignable/readable locations: locals, globals, parameters, object fields, properties, indexes, captures, module exports, receiver places, and access paths. |
| Points-to analysis | Computes which abstract allocation/object/function tokens a place or value may refer to. |
| Alias analysis | Answers whether two places/values may or must refer to overlapping storage/object state. Alias analysis should be a query layer over points-to, value-flow, type facts, and language ownership rules. |
| Access path | A bounded symbolic path such as `req.user.id`, `obj["x"]`, `this.service.client`, or `pkg.Func`. |
| Allocation token | A stable abstract object/function/class/module token that can stand for one or many runtime objects. |
| Summary | A reusable model of a function/method/module/framework boundary: parameters, returns, receiver effects, field writes, thrown values, allocation behavior, call targets, and value-flow edges. |
| Precision tier | A label that says how a fact was derived: exact syntax, type-checker-equivalent, flow-insensitive, flow-sensitive, context-sensitive, summary-modeled, extension-provided, heuristic, or unknown. |

## Standard Implementation Report Template

Every implementation is compared using this structure:

```text
Tool:
Language/domain:
Role:
Key source paths:
Algorithm shape:
Fact model:
Precision:
Cost model:
Strengths:
Weaknesses:
Lessons for polint:
Native implementation implication:
```

## Precision Labels For polint

These labels should be attached to facts and exposed through debug/evidence surfaces:

| Label | Meaning |
|---|---|
| `ExactSyntax` | Derived directly from syntax with no semantic ambiguity. |
| `ExactLanguageSemantics` | Matches a language specification or compiler/type-checker behavior for the modeled construct. |
| `TypeCheckerEquivalent` | Intended to match a reference type checker for a defined compatibility mode. |
| `FlowSensitiveLocal` | Depends on local CFG order and branch facts. |
| `FlowInsensitiveGlobal` | Ignores statement order outside local summaries. |
| `ContextInsensitive` | Merges all call contexts for a function/method. |
| `ContextSensitive(k)` | Separates contexts by call string, object sensitivity, type sensitivity, or similar depth. |
| `SummaryModeled` | Derived from a native or extension-provided summary. |
| `ExtensionProvided` | Added by repo-local Rust extension code. |
| `Heuristic` | Useful but not sound/complete. Must carry a reason. |
| `Unknown` | The engine could not decide within language support, model, or budget. |
| `Unsupported` | The engine intentionally does not model this construct yet. |

## Accuracy Dimensions

Accuracy should not be a single number. Track:

- type precision: how narrow and correct expression/symbol type sets are;
- value precision: constants, nullness, truthiness, enum/string/literal facts;
- points-to precision: average and tail size of points-to sets;
- alias precision: false `MayAlias` rate and justified `MustAlias` rate;
- call-target precision: direct, method, closure, dynamic dispatch, reflection/dynamic property cases;
- flow precision: path/branch/context sensitivity actually used;
- extension delta: default facts versus agent-extended facts;
- unknown honesty: unresolved facts emitted rather than silently hidden.

## Complexity Vocabulary

Use conservative complexity language:

- Type checking is usually near linear in AST/CFG size for common code, but overload resolution, generics, union normalization, structural subtyping, conditional types, and recursive types can create high constants or pathological cases.
- Abstract interpretation is roughly `O(edges * lattice-height)` until widenings/caps intervene.
- Andersen-style points-to is worst-case cubic in classic formulations; practical engines rely on bitsets, SCC collapsing, offline variable substitution, deltas, and sparse graphs.
- Steensgaard-style unification is near-linear but coarse.
- Flow-sensitive points-to can be very expensive if implemented densely over every program point; sparse MemorySSA/SVFG-style designs are the scalable route.
- Context sensitivity must be selective and budgeted. Whole-program high-k context sensitivity is not a default-mode plan.
