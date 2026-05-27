---
phase: 27-layered-module-package-topology-graph
verified: 2026-05-19T14:12:41Z
status: passed
score: 24/24 must-haves verified
overrides_applied: 0
---

# Phase 27: Layered Module/Package/Topology Graph Verification Report

**Phase Goal:** Expand module topology into workspace roots, packages/projects, source sets, declared requirements, lockfile/tool edges, import-to-package facts, and overlays.
**Verified:** 2026-05-19T14:12:41Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

Phase 27 achieved the goal in the actual codebase. The implementation includes crate-private topology contracts and storage, Go and TS/JS collectors, base topology wiring through `polint.module_graph`, semantic-aware import-to-package facts through `polint.module_topology`, cache keys/payloads/validation, eval fixtures, and public no-leak coverage. Review fixes from commit `40a225f` were included in this verification.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Go monorepo module-root inference works. | VERIFIED | `collect_go_topology` uses `GoAnalysisConfig::from_loaded`; `module_graph::go::topology_monorepo` tests passed, including nearest `go.mod` inference without repo-root `go.mod`. |
| 2 | TS/JS package and workspace facts are deterministic. | VERIFIED | `collect_ts_topology`, `parse_package_json`, and lockfile/package-manager evidence paths exist; `module_graph::ts::topology` tests passed. |
| 3 | Import-to-package facts distinguish source, test, generated, vendor, and external where known. | VERIFIED | `derive_import_to_package_edges` maps `SourceSetKind` to import contexts and external/declaration statuses; import-to-package tests passed. |
| 4 | Topology facts participate in relevant cache digests. | VERIFIED | `module_graph_topology_input_digests`, `module_topology_layer_key`, `ShapeKind::ModuleTopology`, schema v2/v1 payloads, and cache tests passed. |
| 5 | Internal topology rows represent workspace roots, packages/projects, source sets, requirements, resolved edges, import-to-package edges, and overlays. | VERIFIED | `TopologyOutput` and all seven row families exist in `module_graph/topology.rs`; `AnalysisDb` stores and accesses all seven families. |
| 6 | Topology row IDs are deterministic run-local IDs assigned from sorted stable keys. | VERIFIED | `TopologyOutput::normalized` sorts and remaps IDs; storage tests assert ID reassignment after replacement. |
| 7 | New topology rows remain crate-private and do not add SDK, runner, crate-root, public CLI, or docs surface. | VERIFIED | No public topology exports found by no-leak greps; `module_topology_internals_stay_private` integration test passed. |
| 8 | Configured `[languages.go].module_roots` are honored before nearest-root discovery. | VERIFIED | Go topology monorepo tests passed for configured module roots overriding nearest discovery. |
| 9 | Go topology rows distinguish module roots, packages, source/test/generated/vendor source sets, declared requirements, replace/exclude directives, and go.sum evidence. | VERIFIED | `go.rs` emits `WorkspaceRootKind::GoModule`, source-set kinds, `DependencyRequirementFact`, go.sum `ResolvedDependencyEdgeFact`, and `MissingLockfile`; Go tests passed. |
| 10 | TS/JS roots derive from package.json, packageManager, workspaces, workspace files, lockfiles, and tsconfig inputs. | VERIFIED | `ts.rs` handles package manifests, pnpm workspace files, package-manager and lockfile overlays, and tsconfig aliases/references; TS topology tests passed. |
| 11 | Dependency sections remain declared requirements and are not collapsed into actual import usage. | VERIFIED | TS dependency rows map package.json sections to `DependencyRequirementFact`; import usage is separately derived in `derive_import_to_package_edges`. |
| 12 | Lockfile evidence records source and schema/version without requiring node_modules or package-manager execution. | VERIFIED | `package_lock.rs` parses package-lock v2/v3 and marks malformed/v1 unsupported; no package-manager execution patterns found in parsers/collectors. |
| 13 | Existing `polint.module_graph` derivation stores base topology rows with resolved imports, module nodes, and module edges. | VERIFIED | `derive_base_topology` merges Go/TS topology and `replace_topology_facts` is called in module graph derivation. |
| 14 | Module graph cache identity changes when topology manifests, lockfiles, workspace files, source-set classifications, or overlay inputs change. | VERIFIED | Topology input digest function hashes `go.mod`, `go.work`, `go.sum`, package and lock files, workspace files, Bun/Yarn/pnpm files, and `tsconfig.json`; key tests passed. |
| 15 | Repo topology overlays produce private facts for generated zones, test-only visibility, internal/public boundaries, source-of-truth directories, ownership, architecture layers, and deploy units where known or explicitly unknown. | VERIFIED | `collect_repo_topology_overlays` and overlay kinds exist; eval fixture asserts generated and test overlays; base topology tests cover unknown overlay categories. |
| 16 | Import-to-package facts bridge syntax imports, resolved imports, Phase 26 semantic import rows, package/project/source-set ownership, and external package nodes. | VERIFIED | `derive_import_to_package_edges` joins `db.imports`, `db.resolved_imports`, `db.semantic_imports`, `db.source_sets`, `db.topology_packages`, and module nodes. |
| 17 | Import-to-package facts distinguish source, test, generated, vendor, external, unresolved, setup-missing, unsupported, dynamic, ambiguous, undeclared, and outside-workspace states. | VERIFIED | Status/context enum variants and classification branches exist; tests cover source/test/generated/vendor, external, dynamic, unresolved, undeclared, outside-workspace, and ambiguous. |
| 18 | Semantic-aware topology facts have their own provider manifest, cache identity, payload restore, and validation after symbol graph execution. | VERIFIED | `polint.module_topology` manifest exists; kernel schedules it after symbol graph; `ModuleTopologyLayerPayload`, cache restore, provider report rows, and validation exist. |
| 19 | Internal eval fixtures assert roots, packages, source sets, declared requirements, resolved dependency edges, overlays, import-to-package rows, Go monorepos, TS/JS workspaces, and cache reuse. | VERIFIED | `module-topology-core` expected TOML asserts all topology families and cache invariants; eval fixture tests passed. |
| 20 | Topology uncertainty statuses are visible in internal eval scoring without adding public topology APIs. | VERIFIED | Eval observation maps topology statuses including `undeclared` and `outside_workspace`; no SDK topology view was promoted. |
| 21 | Public check, inspect rule, polint test JSON, SDK prelude, runner, and docs do not expose new topology internals. | VERIFIED | Public no-leak CLI test passed and source/doc greps found no forbidden public topology terms outside assertions. |
| 22 | Existing public `ResolvedImports` and `ModuleGraphFacts` behavior remains compatible for external rule consumers. | VERIFIED | Temp-repo compatibility rule in `cli.rs` imports only `polint::sdk::prelude::*` and requests those views; focused CLI test passed. |
| 23 | Docs clarify supported relationship facts only without documenting topology internals. | VERIFIED | `docs/facts/imports.md` names `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` as supported relationship surfaces and states richer topology internals are not SDK fact views. |
| 24 | Review fixes from `40a225f` are present in final state. | VERIFIED | Commit `40a225f fix(27)` is in history; code now clears stale empty-import topology rows, emits unsupported package-lock evidence, and matches Go external requirements by module path prefix. |

**Score:** 24/24 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint/src/module_graph/topology.rs` | Internal topology row contracts and deterministic normalization | VERIFIED | Exists, substantive, crate-private topology types and enum/status vocabulary present. |
| `crates/polint/src/core/mod.rs` | AnalysisDb topology storage, replacement APIs, metadata refresh | VERIFIED | `replace_topology_facts`, `replace_import_to_package_facts`, accessors, and provider metadata present. |
| `crates/polint/src/analysis_kernel/metadata.rs` | Topology metadata families | VERIFIED | All topology `FactFamily` variants present. |
| `crates/polint/src/module_graph/formats/go_mod.rs` | Static go.mod parser | VERIFIED | `parse_go_mod` and require/replace/exclude parsing exist. |
| `crates/polint/src/module_graph/formats/go_work.rs` | Static go.work parser | VERIFIED | `parse_go_work` exists with use/replace handling. |
| `crates/polint/src/module_graph/go.rs` | Go topology collector | VERIFIED | `collect_go_topology` emits roots, packages, source sets, requirements, and go.sum/missing-lockfile edges. |
| `crates/polint/src/module_graph/formats/package_json.rs` | Static package.json parser | VERIFIED | `parse_package_json` reads dependency sections, workspaces, packageManager, exports/imports evidence. |
| `crates/polint/src/module_graph/formats/package_lock.rs` | Static package-lock parser | VERIFIED | `parse_package_lock` reads v2/v3 packages and marks malformed/v1 unsupported. |
| `crates/polint/src/module_graph/ts.rs` | TS/JS topology collector | VERIFIED | `collect_ts_topology` emits JS workspace/package/source-set/dependency/lockfile rows. |
| `crates/polint/src/module_graph/mod.rs` | Base and semantic topology wiring | VERIFIED | `derive_base_topology`, `derive_import_to_package_edges`, cache restore, validation helpers, and review fixes present. |
| `crates/polint/src/module_graph/model.rs` | Cache payload schemas | VERIFIED | `module-graph-facts-v2` and `module-topology-facts-v1` payloads present. |
| `crates/polint/src/analysis_kernel/incremental/keys.rs` | Topology-aware cache keys | VERIFIED | Topology input digest and module topology key tests present. |
| `crates/polint/src/analysis_kernel/provider.rs` | Provider manifests | VERIFIED | `polint.module_graph` base outputs and `polint.module_topology` manifest present. |
| `crates/polint/src/analysis_kernel/validation.rs` | Topology validation | VERIFIED | Referential, path, precision, and stable-key validation present. |
| `crates/polint/src/eval/observed.rs` | Internal eval observation | VERIFIED | Reads all topology accessors and emits observed facts. |
| `tests/eval-fixtures/module-topology/core/expected.polint-eval.toml` | Native eval expected rows | VERIFIED | Asserts topology families, statuses, overlays, and cache invariants. |
| `crates/polint/tests/cli.rs` | Public no-leak and SDK compatibility integration tests | VERIFIED | `module_topology_internals_stay_private` passed. |
| `docs/facts/imports.md` | Bounded public relationship docs | VERIFIED | Clarifies supported public relationship facts without topology API promotion. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `core/mod.rs` | `module_graph/topology.rs` | crate-private imports | WIRED | Topology row types imported and stored in `AnalysisDb`. |
| `analysis_kernel/provider.rs` | topology provider outputs | manifest output labels | WIRED | Base outputs and module topology output labels present. |
| `module_graph/go.rs` | Go lifecycle config | `GoAnalysisConfig::from_loaded` | WIRED | Collector uses lifecycle config for root selection. |
| `module_graph/ts.rs` | package manifest parser | `parse_package_json` | WIRED | TS collector calls static manifest parser. |
| `module_graph/mod.rs` | `AnalysisDb::replace_topology_facts` | module graph derivation | WIRED | Base topology is stored during module graph derivation/cache restore. |
| `analysis_kernel/mod.rs` | module topology provider | post-symbol call | WIRED | Kernel calls `derive_module_topology_with_cache_stats` after symbol graph. |
| `module_graph/mod.rs` | semantic imports | semantic-aware classification | WIRED | `derive_import_to_package_edges` calls `db.semantic_imports()` and joins semantic rows. |
| `eval/observed.rs` | AnalysisDb topology accessors | eval observation | WIRED | `topology_facts` reads all seven topology accessor families. |
| `cli.rs` | public CLI JSON | no-leak assertions | WIRED | Public JSON/help/source checks assert forbidden topology markers absent. |
| `cli.rs` | public SDK prelude | external temp-repo rule | WIRED | Compatibility rule consumes `ResolvedImports` and `ModuleGraphFacts`. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `go.rs` | `TopologyOutput` rows | `LoadedConfig`, `AnalysisDb` files, `GoPackageIndex`, `go.mod`, `go.work`, `go.sum` | Yes - static files and package metadata feed rows | FLOWING |
| `ts.rs` | `TopologyOutput` rows | `AnalysisDb` TS/JS files, package manifests, lockfiles, workspace files, tsconfig | Yes - static manifest/lock evidence feed rows | FLOWING |
| `module_graph/mod.rs` base topology | stored base topology rows | Go/TS topology collectors, overlay collector | Yes - merged and stored via `replace_topology_facts` | FLOWING |
| `module_graph/mod.rs` import topology | `import_to_package_edges` | imports, resolved imports, semantic imports, source sets, packages, requirements | Yes - semantic-aware rows stored via `replace_import_to_package_facts` | FLOWING |
| `eval/observed.rs` | observed topology facts | AnalysisDb topology accessors | Yes - eval reads stored rows and emits normalized observations | FLOWING |
| `cli.rs` no-leak test | public JSON/help/source output | real `polint check`, `inspect rule`, and `polint test` commands in temp repo | Yes - public outputs are inspected for forbidden markers | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Go monorepo root/source-set behavior | `cargo test -p polint --lib module_graph::go::topology_monorepo --locked` | 3 passed | PASS |
| TS/JS workspace/package/source-set behavior | `cargo test -p polint --lib module_graph::ts::topology --locked` | 4 passed | PASS |
| Import-to-package classification including review fixes | `cargo test -p polint --lib module_graph::import_to_package --locked` | 5 passed | PASS |
| TS dependency and unsupported package-lock behavior | `cargo test -p polint --lib module_graph::ts::dependency_topology --locked` | 6 passed | PASS |
| Topology cache input participation | `cargo test -p polint --lib module_graph_layer_key_topology_inputs --locked` | 3 passed | PASS |
| Module topology cache restore and empty-import stale-clear fix | `cargo test -p polint --lib module_topology_layer_cache --locked` | 3 passed | PASS |
| Eval fixture and cache invariants | `cargo test -p polint --lib eval::fixtures::module_topology_core --locked` | 3 passed | PASS |
| Public no-leak boundary | `cargo test -p polint --test cli module_topology_internals_stay_private --locked` | 1 passed | PASS |
| Formatting | `cargo fmt --all -- --check` | exit 0 | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SAE-SEM-02 | 27-01 through 27-07 | The module/package/topology graph models workspace roots, packages/projects/source sets, declared requirements, lockfile/tool-resolved edges, import-to-package facts, and repo topology overlays for Go and TS/JS. | SATISFIED | All seven plans declare and address `SAE-SEM-02`; roadmap Phase 27 success criteria all verified through code, tests, eval fixture, cache keys, and public no-leak proof. |

No orphaned Phase 27 requirement IDs were found. `.planning/REQUIREMENTS.md` maps Phase 27 to `SAE-SEM-02`, and all seven plan frontmatters declare `requirements: [SAE-SEM-02]`.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| Multiple test/fixture files | various | empty arrays, fixture `{}`, TODO fixture strings, absent extension placeholder wording | Info | Benign test fixture literals or required absent-placeholder tests; no runtime stub or goal-blocking placeholder found. |

### Human Verification Required

None. This phase is internal Rust/provider/cache/eval behavior with executable tests and source-level verification. No visual, realtime, or external service behavior requires manual confirmation.

### Gaps Summary

No gaps found. The phase goal is achieved, including the review fixes in `40a225f`.

---

_Verified: 2026-05-19T14:12:41Z_
_Verifier: Claude (gsd-verifier)_
