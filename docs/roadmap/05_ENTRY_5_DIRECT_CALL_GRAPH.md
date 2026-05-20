# Entry 5: Direct Call Graph Facts

## Goal

Expose caller-to-callee relationships through public facts, starting with direct
syntactic calls and adding resolved targets over time.

## Why

Call relationships unlock architectural rules, dependency rules, and future
interprocedural analysis.

## Difficulty

**M** for direct syntactic call edges, **XL** for resolved Go/TS call graphs,
**XXL** for precise dynamic-language call graphs.

## What To Build

- `CallEdgeFact`
- `CallResolutionStatus`
- `CallConfidence`
- all call-graph edges query
- calls-from-function query

## Build Method

1. Add call facts with `caller`, `callee_text`, `span`, `resolved_target`,
   `resolution_status`, and `confidence`.
2. Populate direct syntactic calls from existing `FunctionFact::calls`, but add
   spans and call expression kind.
3. For TS/JS, use Oxc semantic symbols/references plus resolved imports.
4. For Go, use `go/packages.Load` with syntax, types, and `TypesInfo`.
5. Make unresolved and dynamic calls explicit instead of hiding them.

## Done When

- Go and TS/JS expose direct call edges.
- Rules can ask for calls from a function.
- Resolved calls include precision/confidence.
- Unresolved calls remain useful evidence.

## Later Languages

- Python should start with lexical/direct call names and import-resolved module
  functions, with dynamic calls marked low confidence.
- Java should use javac `Trees.getElement(TreePath)` or JavaParser symbol solver
  to resolve method invocations.
