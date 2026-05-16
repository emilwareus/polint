# Evaluation Plan For Abstract Domains

## Benchmark Strategy

Use native fixtures for kernel invariants and external suites for transfer
pressure. External suites should not be blindly treated as product truth until
their oracle semantics match polint's facts.

## Benchmark Sources

| Source | Domains | Use |
|---|---|---|
| Go `analysistest`, Go nilness, NilAway | nilness, CFG, SSA-ish facts | Go fixture style and nilness cases. |
| TypeScript compiler tests | nullish, literals, control-flow narrowing | TS/JS narrowing regression cases. |
| test262 | JS runtime semantics | Parser/semantic edge-case pressure, not diagnostic oracle. |
| Pyright samples, mypy test data, Python typing conformance | None, literal, TypeGuard, TypeIs, TypedDict | Python transfer/reference behavior. |
| Checker Framework tests | nullness, value, initialization, must-call | Java domain fixtures. |
| Clang Static Analyzer tests | path sensitivity, ranges, resource states | Expected-warning/eval patterns. |
| Infer tests | resource, nullness, taint, summaries | Industrial summary/path-sensitive patterns. |
| ValBench | exact strings/constants | String/value precision measurement. |
| Juliet/SARD | null deref, resource, overflow | Large synthetic security corpus. |
| SV-COMP / sv-benchmarks | numeric/path sensitivity | Nightly external-transfer suite. |
| TAJS/Jelly tests | JS abstract interpretation/call graph | JS dynamic behavior pressure. |
| BugsJS | real JS bug corpus | Executable oracle for selected cases. |
| DroidBench/TaintBench | taint | Taint design transfer after dataflow domains mature. |

## Metrics

| Metric | Meaning |
|---|---|
| Diagnostic TP/FP/FN | Match by rule id, file, span tolerance, and message class. |
| Fact inclusion | Expected concrete value set must be included in abstract value. |
| Fact precision | Smaller abstract values score better after inclusion is satisfied. |
| Top rate | Percent of facts degraded to top/unknown. |
| Unsupported rate | Percent of facts marked unsupported/setup-missing/budget-exceeded. |
| Path precision | Cases requiring branch correlation, loop guards, aliasing, summaries. |
| Runtime | Wall time, CPU time, files/sec, LOC/sec. |
| Memory | Peak RSS and per-domain store sizes. |
| Cache speedup | Cold vs warm runs and invalidation correctness. |
| Determinism | Stable JSON/fact ids across worker counts and file order. |
| Extension delta | Precision/recall/top-rate change after agent-authored extensions. |

## Ground Truth Tiers

1. Inline source oracle: `// want`, `expected-warning`, `# E`, and
   `polint-expect` fact comments.
2. Golden JSON oracle: checked snapshots for facts, summaries, and diagnostics.
3. Executable oracle: tests that prove injected bug/value behavior.
4. External labeled/reference corpora: Juliet/SARD/SV-COMP/DroidBench labels.

Agent-generated labels are proposals, not truth. Promote only after executable
validation, reference analyzer agreement, or human review.

## Default CI Suite

| Layer | Contents | Gate |
|---|---|---|
| Domain law tests | lattice laws, serialization, stable hashes | pass |
| Core fact fixtures | Go and TS/JS snippets for P0 domains | zero unexpected diffs |
| Temp-repo rule fixtures | public SDK imports only | diagnostics match JSON |
| Extension fixtures | guard and summary model examples | validation and output pass |
| Crash corpus | syntax errors, generated/minified code, unsupported features | no panics |
| Parallel determinism | different worker counts and file order | byte-identical JSON |

## Nightly / Pre-Release Suite

| Layer | Contents | Gate |
|---|---|---|
| External slices | TypeScript narrowing, Go nilness, Clang eval, Checker nullness, ValBench strings | report deltas |
| Differential analyzers | compare with go vet/NilAway, TS compiler, Pyright, Clang/Infer when relevant | triage disagreement |
| Agent mutants | semantics-preserving rewrites and bug injections | executable oracle passes |
| Larger corpora | Juliet/SARD/SV-COMP/BugsJS selected cases | report-only until adapted |
| Performance repos | fixed medium/large repos | no material regression |

## First Benchmark Slice

Start with:

- native Go nilness fixtures;
- TS/JS nullish/truthiness/literal narrowing fixtures;
- string literal/template fixtures;
- resource typestate mini fixtures;
- Clang-style `eval` fact assertions adapted to `polint-expect`;
- cache/determinism tests.

Do not wait for huge external suites before implementing the first domain. Use
external suites to harden and compare after the native fixture format exists.
