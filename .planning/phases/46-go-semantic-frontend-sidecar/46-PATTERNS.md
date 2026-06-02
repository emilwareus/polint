# Phase 46: Go Semantic Frontend & Sidecar - Pattern Map

**Generated:** 2026-06-01
**Source:** inline pattern mapping from 46-CONTEXT.md

## Target File Map

| Planned area | New or modified files | Closest existing analogs |
|--------------|-----------------------|--------------------------|
| Go semantic sidecar source | `crates/polint/go-sidecar/polint-go-frontend/{go.mod,go.sum,main.go,internal/semantic/emit.go,internal/semantic/emit_test.go}` | `crates/polint/go-sidecar/polint-go-symbols/` |
| Rust protocol and process client | `crates/polint/src/go/semantic/{mod.rs,protocol.rs,client.rs,process.rs,tests.rs}` | `crates/polint/src/symbol_graph/go.rs`, `crates/polint/src/go/lifecycle.rs` |
| Go semantic fact storage and lowering | `crates/polint/src/go/semantic/{facts.rs,store.rs,lower.rs,cache_key.rs,validate.rs}` | `crates/polint/src/ts/binding/`, `crates/polint/src/analysis/semantic_graph/`, `crates/polint/src/analysis/reachability/` |
| Semantic graph integration | `crates/polint/src/analysis/semantic_graph/{build.rs,provider.rs,cache_key.rs,validate.rs,debug.rs}` | Phase 45 TS direct-binding projection and Phase 44 semantic graph provider |
| Identity and benchmark integration | `crates/polint/src/analysis/identity/provider.rs`, `crates/polint/src/analysis/identity/render/go_relstring.rs`, `crates/polint/src/eval/external/go_x_tools_callgraph.rs` | Phase 42 import-path deferral comments and renderer tests |
| Failure taxonomy and verification | `crates/polint/src/go/semantic/*`, `crates/polint/tests/public_surface_leak.rs` (assert unchanged), semantic graph/eval fixtures | `symbol_graph::go` failure tests, `eval/determinism_gate.rs`, public leak gate |

## Existing Patterns To Follow

### Sidecar Packaging

- `symbol_graph::go` uses a schema constant, an env override, installed binary detection next to the current executable, embedded source materialization into a versioned temp directory, source drift tests, and path validation.
- The existing Go sidecar source uses `go/packages` with `NeedSyntax`, `NeedTypes`, `NeedTypesInfo`, `NeedTypesSizes`, and `NeedModule`; Phase 46 should add `go/ssa` and richer semantic rows, not replace the current symbol schema.
- Existing sidecar errors are broad `SetupMissing`; Phase 46 should keep controlled failure conversion but preserve distinct categories required by GO-04.

### Go Lifecycle

- `go::lifecycle::GoAnalysisConfig` already handles `module_roots`, `package_patterns`, `build_tags`, `include_tests`, `offline`, nearest-`go.mod` inference, checked-in `go.work`, and temp synthetic workspaces.
- New sidecar requests should consume that lifecycle object directly. Avoid new config files, hidden generated repo files, or a separate package-pattern model.

### Semantic Graph And Cache

- `analysis::semantic_graph::provider` digests provider/schema parameters, config, upstream provider digests, lifecycle components, and stable graph output keys. Go semantic output must join this digest chain without using dense IDs.
- Phase 45 already projects TS direct bindings into `CopyEdge` and `CallConstraint`; Phase 46 should mirror this with Go callsite obligations while keeping interface/RTA resolution deferred.
- Validation must reject dangling endpoints and preserve honest precision/status labels rather than silently dropping broken rows.

### Rust Code Style

- Use borrowed parameters (`&str`, `&[T]`, `&Path`) unless ownership transfer is required.
- Use typed error enums for library/client failures; avoid `anyhow` outside binaries and tests.
- Keep production paths panic-free. `unwrap`/`expect` are acceptable only in tests or impossible states with clear assertions.
- Add comments only for non-obvious process/protocol guarantees, such as why a child process must be killed/waited in a specific order.

## Planning Constraints

- All new Rust modules remain `pub(crate)` or private.
- The existing `polint-go-symbols` behavior must remain compatible unless a plan explicitly updates its docs/tests.
- `golang.org/x/tools v0.45.0` is a Phase 46 acceptance requirement for `polint-go-frontend`.
- No solver-derived Go RTA edges are emitted in Phase 46.
- Public SDK and public graph/solver CLI surfaces remain out of scope.
