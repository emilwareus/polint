# Standard: Incremental Query And Cache System Review

Use this standard when reviewing future incremental systems, caches, query
engines, or analyzer-daemon designs for polint.

## Vocabulary

| Term | Meaning |
|---|---|
| Input snapshot | Canonical view of source files, config, toolchain facts, manifests, lockfiles, rule options, extension code, and generated model inputs for one run. |
| Source digest | Hash of exact file content or in-memory overlay content. |
| Shape digest | Hash of the source shape that affects downstream facts, such as imports, exported symbols, public API, declarations, or summary effects. |
| Layer | A named fact family produced by a provider, such as syntax, imports, module graph, symbols, CFG, call graph, summaries, data flow, evidence, or diagnostics. |
| Layer key | Stable cache key for a layer output: provider identity, schema, inputs, lifecycle, config, extension digests, and dependency layer digests. |
| Query | Demand computation over layers or other queries, usually for expensive or rule-specific facts. |
| Query key | Stable identity for a query result: query kind, version, parameters, relevant layer digests, budget, precision tier, and options. |
| Dependency index | Reverse map from inputs/layers/queries/summaries/extensions to dependents that must be verified, recomputed, or dropped after changes. |
| Invalidation plan | Deterministic list of reuse, verify, recompute, drop, or quarantine actions for affected layers and queries. |
| Red-green verification | Reuse strategy where unchanged dependencies make a query green; dirty dependencies trigger recursive verification or recompute. |
| Backdating | If a recomputed value equals the previous value, record it as logically unchanged so downstream dependents can remain valid. |
| Durability | Classification of how often an input changes and how widely it should invalidate results. |
| Quarantine | State for cached results that depend on changed, failed, unvalidated, or undeclared extension/model inputs. |
| Precision tag | Statement of semantic strength, such as exact, conservative, heuristic, under-approximate, timeout-truncated, or extension-asserted. |

## Required Review Questions

Every incremental design must answer:

- What are the stable input keys?
- Which source edits are content-only, syntax-shape, public API, module
  topology, summary-effect, or diagnostic-only changes?
- What input digests are included in every cache key?
- What dependencies are recorded while computing each result?
- Which cached outputs can be verified without recompute?
- Which outputs require recompute even when hash keys look unchanged?
- How are equality and backdating handled?
- How are recursive SCCs handled?
- How are rule options and rule code digests represented?
- How are Rust-code extension crates, generated model files, and extension-read
  files declared and invalidated?
- What happens when an extension reads an undeclared input?
- What happens when an extension changes precision, provenance, or validation
  status without changing emitted fact values?
- How is stale cache data prevented after schema/provider/toolchain changes?
- How are diagnostics reused, hidden, dropped, or re-fingerprinted?
- How are cache hits and misses benchmarked?
- How does the design fail closed when invalidation is uncertain?

## Complexity Reporting Template

Use this format for reports:

```text
Hot cache hit:
  O(number of key digests + manifest validation)

Red-green query verify:
  O(number of previous dependency edges visited)

Cold recompute:
  O(provider/query-specific cost)

Invalidation planning:
  O(changed inputs + reverse dependency closure)

Recursive SCC update:
  Best case: O(affected SCC boundary)
  Conservative first implementation: O(affected SCC closure recompute)
  Worst case: O(whole analysis family)

Memory:
  layer outputs + dependency edges + manifests + memoized query results
```

Do not report only asymptotic complexity. Static-analysis engines are dominated
by constants, graph density, parser/type-checker cost, relation sizes, and
memory locality.

## Accuracy Reporting Template

Incremental accuracy means two things:

- semantic accuracy: the facts are as precise/sound as the provider claims;
- cache accuracy: reused facts are still valid for the current inputs.

Reports should distinguish:

| Mode | Required claim |
|---|---|
| Exact reuse | Output digest and all declared dependencies are unchanged. |
| Verified reuse | A dirty dependency was checked and determined logically unchanged. |
| Backdated reuse | Recomputed value equals old value; downstream dependents need not change. |
| Conservative recompute | The engine cannot prove reuse, so it recomputes. |
| Quarantined | A dependency or extension input is not trusted. |
| Dropped | The old output cannot be reused or verified. |

## Polint-Specific Additions

The review must explicitly cover:

- default mode versus agent-extended mode;
- extension digests and extension-read files;
- provenance and validation status as cache inputs;
- unknown and unsupported facts as cacheable outputs;
- official language tool invocations as cacheable provider inputs;
- public SDK stability and hidden internal query shape;
- reproducible benchmark fixtures for cache behavior.
