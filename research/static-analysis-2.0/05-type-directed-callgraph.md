# 05 — Type-Directed Callgraph (tiered resolution)

## Problem

We spent ~50 iterations teaching an Andersen heap to resolve *untyped*
JavaScript, because the Jelly benchmark is untyped-JS-heavy. Our actual
market — enterprise repos, AI-agent workflows — is overwhelmingly
**TypeScript and Go: typed languages**. Type information resolves the
majority of real-world call sites near-precisely at ~linear cost, and polint
uses none of it for TS today. This is the single highest-leverage strategic
correction from the review, and TS type-directed CG construction is
under-published — a genuine novelty angle.

## The tier model

Resolve every call site with the cheapest sufficient tier; escalate only on
residue:

1. **Direct/lexical** — identifier/import binding (exists today).
2. **Type-directed (XTA-grade)** — receiver type's member lookup, narrowed
   by the type checker; Go: `go/types` via the sidecar; TS: tsc-provided
   types via a new type sidecar. Near-linear; covers most enterprise code.
3. **Field-based (ACG-style)** — one abstract location per property name;
   nearly linear, F≈95 on small JS (Feldthaus et al., ICSE 2013). Fallback
   where types are absent/`any`.
4. **Points-to heap** — the existing Andersen solver, reserved for the
   untyped/higher-order residue only.
5. **Verified ML** — type/shape inference + callee ranking on what's left
   (doc 07).

Each edge carries which tier produced it (honesty metadata) — rules can
choose their precision floor. Tiering is also the cost-control pattern from
Scaler (FSE 2018) and Tricorder: bound worst-case cost repo-wide, spend
precision only where needed.

## Current state in polint

- Go: tree-sitter syntax + RTA in `go/semantic/` (F1 92.5% on x/tools) —
  already effectively tier 2 for Go; sidecar pattern proven
  (`go/semantic/client.rs`, spawned via `Command::new("go")` in
  `go/lifecycle.rs:615`; standalone Go binary in `tools/polint-go-symbols/`).
- TS/JS: oxc gives syntax + bindings but **no type checker**; tiers 2–3 do
  not exist; everything untyped goes to recognizers/value-flow/heap.
- The internal type-alias-points-to research track already recommends
  "official language tooling as provider inputs with provenance".

## What the research says

- **Propagation-based CG hierarchy** — Tip & Palsberg, OOPSLA 2000
  (http://web.cs.ucla.edu/~palsberg/paper/oopsla00.pdf): CHA → RTA → XTA;
  RTA classifies ~92% of virtual sites monomorphic in typed OO code; XTA
  shrinks type sets ~88% vs RTA at ~12× CHA cost — all far cheaper than
  points-to.
- **ACG** — Feldthaus, Schäfer, Sridharan, Dolby, Tip, ICSE 2013: field-based
  approximate CGs for JS IDE services; deliberately unsound, nearly linear,
  the only approach (with Closure) that survives M-LOC inputs in the 2024
  comparative study (arXiv:2405.07206). Implementation reference:
  https://github.com/Persper/js-callgraph.
- **TS-specific gap**: no widely-cited system builds CGs from tsc types at
  scale — the literature is either untyped-JS points-to (Jelly/TAJS) or
  IDE-grade ACG. Building tier 2 for TS and publishing numbers is open
  ground.
- **tsgo** (microsoft/typescript-go): the native tsc port (~10× faster
  checking) makes a type sidecar operationally cheap; fallback is a Node
  script on the TypeScript compiler API (same architecture as
  polint-go-symbols uses go/packages).

## Direction for polint

1. Prototype a TS type sidecar emitting, per call site: resolved signature,
   receiver type, member-resolution result (JSON protocol, mirroring the Go
   sidecar client). Node+TS-compiler-API first; evaluate tsgo when its API
   surface allows.
2. New provider `polint.ts.types` feeding the calls provider a typed
   resolution tier **before** value-flow/heap; heap consumes only the
   residue (also shrinks its constraint set → memory/speed win).
3. Tier labels on call edges; benchmark on typed real-world TS repos
   (doc 01 corpus must include several).
4. Go: keep RTA; consider XTA-grade set narrowing if precision numbers on
   real Go repos demand it.
5. Monorepo handling: respect tsconfig project references / path mapping
   (module_graph already models workspace topology — reuse it to scope
   sidecar invocations).

## References

Tip & Palsberg OOPSLA'00 · ACG ICSE'13 · comparative study arXiv:2405.07206 ·
Scaler FSE'18 · typescript-go: https://github.com/microsoft/typescript-go ·
TS compiler API: https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API
· internal: research/type-alias-points-to/, tools/polint-go-symbols/
