# Repository Index

No external repositories were cloned for this track. The task was a deep design
review of polint's local Rust implementation. External OSS algorithm
implementations are indexed in the previous tracks, especially
`research/call-graphs/`, `research/data-flow/`, `research/analysis-kernel/`,
`research/type-alias-points-to/`, `research/effects-summaries/`, and
`research/abstract-interpretation/`.

## Local Repository Files Inspected

| Path | Why inspected | Key findings |
| --- | --- | --- |
| `Cargo.toml` | Workspace Rust design and dependency/lint baseline. | Rust 2024, `rust-version = "1.95"`, curated workspace dependencies, `unsafe_code = "forbid"`, `unreachable_pub = "deny"`, and useful clippy lints already exist. |
| `crates/polint/src/lib.rs` | Public/private module boundary. | Supported surface is narrow: `runner`, `sdk`, and `rule`; internal modules are `pub(crate)` with a hidden `_bench` feature surface. This is the right public API posture. |
| `crates/polint/src/core/mod.rs` | Current fact model, `AnalysisDb`, capabilities, rules, cache restore. | Good small ID pattern and SDK-facing facts, but `AnalysisDb` is already broad. Adding MIR/place/summary/domain facts directly here would create a large coupling point. |
| `crates/polint/src/sdk/facts.rs` | Typed view pattern. | The borrowed `Copy` fact views are a strong pattern: rule ergonomics stay simple while internals remain private. Reserved `Cfg`, `CallGraph`, and `DataFlow` views should remain empty until supported. |
| `crates/polint-macros/src/lib.rs` | Capability derivation. | Macro-derived capabilities already map future views (`Cfg`, `CallGraph`, `DataFlow`) to capability names. This supports delayed implementation without handwritten rule capabilities. |
| `crates/polint/src/analysis_plan.rs` | Planning, capability status, deterministic digest. | Existing plan is the natural control point for provider scheduling and cache identity. It already catches rule metadata/capability panics and emits unsupported capability diagnostics. |
| `crates/polint/src/cache/keys.rs` | Deterministic cache digest encoding. | Manual deterministic encoders are the right precedent for semantic artifact keys. Extend this approach rather than relying on serializer map ordering. |
| `crates/polint/src/cache/mod.rs` | Cache layout and file cache behavior. | Existing file cache is useful for syntax facts. Semantic summaries and extension facts need artifact/layer keys, dependency digests, and invalidation records. |
| `crates/polint/src/go/adapter.rs` | Go parsing lifecycle and cache/parallelism pattern. | Per-file local `AnalysisDb`, thread-local parser, sorted parallel results, and restore step are good patterns. Current Go facts are syntactic only. |
| `crates/polint/src/ts/adapter.rs` | TS/JS parsing lifecycle and AST lifetime constraints. | Oxc allocator lifetimes stay local. Semantic MIR must own normalized facts and not borrow Oxc AST nodes. |
| `crates/polint/src/module_graph/model.rs` | Deterministic builder and merge pattern. | Builder with `BTreeMap`/`BTreeSet`, draft types, finish sorting, and ID reassignment is a useful model for new fact builders. |
| `crates/polint/src/symbol_graph/model.rs` | Stable-key builder and collision diagnostics. | Strongest local reference for future semantic facts: stable keys, collision detection, precision fields, deterministic output sorting. |
| `crates/polint/src/symbol_graph/stable_id.rs` | Stable identity encoding. | Length-prefixed normalized stable keys are a good pattern for cache/provenance identity. Consider generalizing the pattern internally before duplicating it per fact family. |
| `crates/polint/src/symbol_graph/query.rs` | Query helper pattern. | Internal query helpers mirror SDK views without exposing storage. Use this for call/place/summary queries. |
| `crates/polint/src/module_graph/query.rs` | Graph traversal helper pattern. | Small deterministic algorithms over fact slices are preferable to exposing graph internals. |
| `crates/polint/src/runner/mod.rs` | End-to-end orchestration. | Current phase order is hardcoded: load files, parse Go/TS, derive module graph, derive symbols, derive metrics, run rules. Provider DAG should evolve here first. |
| `crates/polint/tests/cli.rs` | External-rule behavior tests. | Temp-repo tests already enforce public SDK usage and unsupported capability behavior. This is the gate pattern for future public views. |

## Caveats

- This was not a bug audit of all existing code.
- No external OSS repositories were cloned because the algorithmic source-code
  inspection already exists in prior research tracks.
- The report recommends architecture, not immediate code changes.
