# Native Rust Implementation Path

## First Vertical Slice

Build a thin but real module graph substrate before adding deep language-specific exactness.

Target:

```text
Root discovery
  + package facts
  + declared dependency facts
  + lockfile exact facts for common managers
  + Go full native tier
  + TS/JS workspace and import ownership tier
```

## Suggested Crate Boundaries

Start internal inside `crates/polint/src/module_graph`. Split crates only when the module becomes too large or useful to external rule packs.

```text
module_graph/
  facts.rs          typed fact records
  ids.rs            typed IDs and stable keys
  discovery.rs      root discovery
  manager.rs        package manager detection
  parse/            format parsers
  providers/        ecosystem providers
  index.rs          package/file/import indexes
  validation.rs     referential and precision validation
  cache_key.rs      deterministic inputs
```

## Rust Data Shape

```rust
pub(crate) struct PackageId(u32);
pub(crate) struct WorkspaceRootId(u32);
pub(crate) struct DependencyRequirementId(u32);
pub(crate) struct ResolvedDependencyId(u32);

pub(crate) enum PackageManager {
    GoModules,
    Npm,
    Pnpm,
    YarnClassic,
    YarnBerry,
    Bun,
    Pip,
    Uv,
    Poetry,
    Pdm,
    Conda,
    Maven,
    Gradle,
    Bazel,
    Pants,
    Cargo,
    Unknown,
}
```

Keep public visibility narrow per repo conventions. Do not expose these as stable SDK types until the API is intentionally promoted.

## Parser Strategy

Use structured parsers:

- JSON: existing serde_json stack.
- TOML: existing toml stack.
- XML: add a small XML parser only when Maven implementation begins.
- YAML: decide between a small dependency or a strict subset parser for pnpm/Python environment files.
- Gradle/Starlark/BUILD: static recognizer first, not full language parser.

Every parser should produce source spans where practical. Spans are valuable for diagnostics and agent repair.

## Data Indexes

Build deterministic indexes after fact validation:

```text
workspace path -> WorkspaceRootId
manifest path -> PackageId
package stable key -> PackageId
package name + ecosystem + manager -> PackageId list
file path -> owning PackageId/SourceSetId
declared requirement -> requirement id
from package -> dependency edges
to package -> reverse dependency edges
import fact -> import-to-package fact
```

Use sorted vectors/maps first. Optimize representation after benchmarks.

## Extension Integration

Module graph extensions should use the analysis-kernel provider model:

```rust
pub trait ModuleGraphProvider {
    fn describe(&self) -> ProviderDescriptor;
    fn provide(&self, input: ModuleGraphInput<'_>, sink: &mut ModuleGraphSink<'_>) -> Result<()>;
}
```

Potential sink methods:

```rust
sink.add_workspace_root(...);
sink.add_package(...);
sink.add_dependency_requirement(...);
sink.add_resolved_dependency(...);
sink.add_source_set(...);
sink.add_import_resolution(...);
sink.add_topology_tag(...);
sink.mark_unsupported_dynamic(...);
```

Validation should reject:

- facts pointing outside allowed repo roots unless marked external;
- duplicate stable keys without conflict policy;
- exact facts without enough input provenance;
- topology tags targeting missing packages/files;
- overrides of exact native facts without explicit conflict records.

## Implementation Order

1. Add internal fact structs and arenas.
2. Add root discovery with tests for nested mixed-language repos.
3. Add `package.json`, `go.mod`, `go.work`, `pyproject.toml`, `pom.xml`, and `Cargo.toml` manifest fact extraction.
4. Add dependency requirement extraction for Go, JS, Python, Maven, Cargo.
5. Add package-manager detection and diagnostics.
6. Add npm/pnpm/Yarn/Bun lockfile exact readers.
7. Add uv/Poetry/PDM/Cargo lockfile readers.
8. Add Go native MVS and import path ownership.
9. Add TS/JS workspace ownership and tsconfig path import resolution.
10. Add extension provider sink and merge validation.
11. Add public `Packages<'_>` and `Dependencies<'_>` views.

## Acceptance Gate Before SDK Promotion

Before exposing public SDK views:

- at least Go and TS/JS facts work on real repos;
- fact precision labels are documented;
- missing lockfile/dynamic unsupported cases emit diagnostics;
- extension merge and validation are tested;
- cache keys include all manifest/lockfile/config inputs;
- temp-repo tests mimic external rule authors using only public SDK.
