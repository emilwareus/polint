# Standard Module Graph Vocabulary

This file defines the comparison vocabulary used across this research package.

## Implementation Profile

Each report uses these fields:

| Field | Meaning |
|---|---|
| Ecosystem scope | Go, TS/JS, Python, Java/JVM, Rust, monorepo/build-system, or multi-language. |
| Semantic unit | Repository, workspace, module, package, project, source set, target, crate, distribution, artifact, environment. |
| Input formats | Manifest, lockfile, workspace file, build file, tool config, generated loader, environment metadata. |
| Graph objects | Root, package, module, project, target, source set, artifact, dependency requirement, resolved dependency, import edge. |
| Identity model | How nodes stay stable across paths, package managers, versions, environments, and runs. |
| Resolution source | Manifest, lockfile, package manager algorithm, build tool query, import resolver, extension provider. |
| Precision strategy | Exact, lockfile exact, tool reported, conservative, heuristic, dynamic, extension asserted, unsupported. |
| Incrementality model | File digest, manifest/lockfile digest, provider cache key, build-system graph cache, query dependency tracking. |
| Failure modes | Dynamic scripts, generated code, peer dependencies, platform markers, classpaths, variants, path aliases, missing lockfiles. |
| Polint implication | What to copy, what to avoid, and what fact layer should represent it. |

## Fact Families

### `WorkspaceRootFact`

```text
WorkspaceRootFact {
  id: WorkspaceRootId,
  path: RepoPath,
  language: Option<Language>,
  ecosystem: Ecosystem,
  manager: PackageManager,
  root_kind: ExplicitWorkspace | SingleProject | BuildRoot | ModuleRoot | VirtualRoot | ExtensionRoot,
  manifest_files: Vec<FileId>,
  lock_files: Vec<FileId>,
  config_files: Vec<FileId>,
  discovery_status: Exact | Ambiguous | Heuristic | ExtensionAsserted | Unsupported,
  provenance: ProvenanceId,
}
```

### `PackageFact`

```text
PackageFact {
  id: PackageId,
  stable_key: StablePackageKey,
  workspace_root: WorkspaceRootId,
  ecosystem: Ecosystem,
  manager: PackageManager,
  name: Option<String>,
  version: Option<String>,
  path: RepoPath,
  manifest: Option<FileId>,
  lockfile: Option<FileId>,
  package_kind: WorkspaceMember | External | Root | Virtual | Generated | BuildTarget | SourceSet,
  lifecycle: LifecycleInputsId,
  precision: Precision,
  provenance: ProvenanceId,
}
```

### `SourceSetFact`

Used when package membership is not enough, especially Java/JVM, Gradle, Bazel, Pants, and generated code.

```text
SourceSetFact {
  id: SourceSetId,
  package: PackageId,
  kind: Main | Test | IntegrationTest | Generated | Resource | Tool | Custom(String),
  roots: Vec<RepoPath>,
  includes: Vec<Glob>,
  excludes: Vec<Glob>,
  build_tags_or_conditions: Vec<Condition>,
  classpath_or_module_path: Vec<PackageId>,
  provenance: ProvenanceId,
}
```

### `DependencyRequirementFact`

Declared dependency before resolution.

```text
DependencyRequirementFact {
  id: DependencyRequirementId,
  from_package: PackageId,
  raw_name: String,
  normalized_name: String,
  ecosystem: Ecosystem,
  requested: DependencySpec,
  kind: Runtime | Dev | Test | Build | Peer | Optional | Plugin | Tool | AnnotationProcessor | Constraint | Override,
  groups: Vec<String>,
  marker: Option<MarkerExpression>,
  source_span: Option<SourceSpan>,
  status: Parsed | Invalid | Dynamic | Unsupported,
  provenance: ProvenanceId,
}
```

### `ResolvedDependencyFact`

Selected edge after lockfile, package-manager resolution, build-tool query, or extension.

```text
ResolvedDependencyFact {
  id: ResolvedDependencyId,
  requirement: Option<DependencyRequirementId>,
  from_package: PackageId,
  to: ResolvedTarget,
  selected_version: Option<String>,
  edge_kind: Runtime | Compile | Test | Dev | Peer | Optional | Build | Tool | SourceSet | Target | ImportOnly,
  resolution_source: ManifestDeclared | Lockfile | NativeResolver | ToolReported | ImportResolver | Extension,
  status: Exact | Conservative | Ambiguous | Missing | Excluded | PlatformConditional | Dynamic | Unsupported,
  conditions: Vec<Condition>,
  provenance: ProvenanceId,
}
```

### `ImportToPackageFact`

The bridge from semantic imports to module/package graph.

```text
ImportToPackageFact {
  import_fact: ImportId,
  from_file: FileId,
  from_package: Option<PackageId>,
  specifier: String,
  resolved_module: Option<ModuleId>,
  resolved_package: Option<PackageId>,
  external_name: Option<String>,
  status: Exact | Ambiguous | External | Missing | Dynamic | Unsupported,
  precision: Precision,
  provenance: ProvenanceId,
}
```

### `RepoTopologyFact`

Architecture facts that are not package-manager facts but are essential to policy rules.

```text
RepoTopologyFact {
  subject: PackageId | SourceSetId | FileId | DirectoryId,
  layer: Option<String>,
  owner: Option<String>,
  deploy_unit: Option<String>,
  generated_zone: Option<String>,
  visibility: Option<VisibilityPolicy>,
  tags: Vec<String>,
  source: Config | Heuristic | Extension | ToolReported,
  provenance: ProvenanceId,
}
```

## Precision Labels

| Label | Definition |
|---|---|
| `ManifestExact` | Parsed from a static manifest field with known semantics. |
| `LockfileExact` | Parsed from a lockfile entry that pins a selected package/version/context. |
| `ToolReported` | Reported by an external build/package tool oracle, if enabled. |
| `NativeResolved` | Resolved by polint's native implementation of that ecosystem's algorithm. |
| `ImportExact` | Source import was resolved to an exact package/module under selected lifecycle inputs. |
| `Conservative` | Over-approximates possible roots, packages, or edges. |
| `Heuristic` | Useful but may be incomplete or wrong. |
| `Dynamic` | Runtime/build-time execution can affect the answer. |
| `ExtensionAsserted` | Emitted by repo-local Rust extension before validation. |
| `ExtensionValidated` | Extension fact passed validation gates. |
| `Unsupported` | Recognized feature not yet modeled. |

## Stable Identity

Use stable package keys that include the ecosystem and lifecycle:

```text
go:module_path@version in workspace root
js:package_name@version under manager+lockfile+peer_context
python:normalized_distribution@version under environment marker set
jvm:group:artifact:classifier:extension:version under source_set/classpath
rust:cargo_package_name@version with source and features
bazel://repo//package:target under configuration when known
pants:address under resolve/environment
```

Do not use only a directory path as a package key. Paths collide across generated packages, vendored packages, workspaces, source sets, and build targets.

## Complexity Vocabulary

Use:

- `F`: repository files or directories walked.
- `M`: manifests.
- `L`: lockfile bytes or entries.
- `P`: package/project nodes.
- `R`: declared requirements.
- `E`: resolved dependency edges.
- `I`: source import specifiers.
- `C`: conditions, variants, peer contexts, markers, or configurations.
- `T`: build targets.

Typical costs:

- Root scan: `O(F)` with ignore pruning.
- Manifest parse: `O(total manifest bytes)`.
- Workspace glob expansion: `O(candidate dirs + glob matches)`, not `O(all files)` if directory pruning is used.
- Lockfile parse: `O(L + P + E)`.
- Declared graph build: `O(P + R)`.
- Lockfile graph build: `O(P + E)`.
- Native solving: can be exponential in worst case for SAT/backtracking/PubGrub-style solvers; Go MVS is graph traversal over requirements.
- Import-to-package resolution: usually `O(I * path_depth * candidate_extensions)` with caches, plus package `exports` and path alias conditions.
- Transitive closure: `O(P + E)` for each requested root if cached, or one all-pairs relation if needed.

## Pseudo-Code Style

Use Python-ish pseudo-code:

```python
def build_module_graph(repo):
    roots = discover_workspace_roots(repo)
    manifests = parse_manifests(roots)
    requirements = collect_declared_requirements(manifests)
    resolved = parse_lockfiles_or_resolve(requirements)
    imports = resolve_imports_to_packages(repo.semantic_imports, manifests, resolved)
    topology = merge_repo_topology(manifests, imports, extensions)
    return validate_and_index(roots, manifests, requirements, resolved, imports, topology)
```

The Rust implementation should use typed IDs, arenas, deterministic sorted iteration, sidecar metadata, explicit unknown facts, and analysis-kernel provider scheduling.
