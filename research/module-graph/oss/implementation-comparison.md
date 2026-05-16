# OSS Implementation Comparison

| Tool | Primary Graph | Resolver Style | Incrementality/Cache | What Polint Should Copy |
|---|---|---|---|---|
| Go command | Module graph/build list | MVS highest-minimum traversal | Module cache, lazy loading, go.work lifecycle | Native Go resolver and explicit lifecycle inputs. |
| gopls | Workspace modules and package metadata | Delegates package loading to Go tooling | Snapshot cache and metadata reload | Workspace root discovery and diagnostics. |
| npm Arborist | Install tree with Node/Edge inventory | Ideal tree plus hoisting/peer placement | Lockfile/actual/virtual tree reuse | Edge/node model and peer/optional/platform statuses. |
| pnpm | Workspace graph and virtual store | Range matching plus strict workspace protocol | Shared lockfile and virtual store | Workspace protocol semantics and strict dependency access. |
| Yarn Berry | Project/workspace/locator graph, PnP map | Resolver/fetcher/linker plugins | Install state, lockfile, PnP loader | No-node_modules dependency map and plugin architecture. |
| Bun | Package manager lock/install graph | npm-compatible with Bun-specific install strategies | Bun lock/cache | Workspace/lockfile support and isolated/hoisted mode labels. |
| TypeScript | Module resolution/project graph | Host-aware Node/TS resolver | Builder state/project references | Import resolution bridge and project reference semantics. |
| Oxc resolver | Node/TS file/package resolver | Rust-native resolver pipeline | Path/cache structures | Rust implementation patterns for TS/Node resolution. |
| pip/resolvelib | Requirements/candidates/result graph | Backtracking provider API | Round limit, provider cache | Provider abstraction and explicit result graph. |
| uv | Python workspace/resolution/lock graph | PubGrub with marker/universal support | Workspace cache keys, batch prefetch, lockfile | Rust-native design and marker/environment preservation. |
| Poetry | Package dependency solve and lock graph | Mixology/PubGrub-style with overrides | Locked package reuse | Marker aggregation and override retry awareness. |
| PDM | Requirements/candidates/lock graph | resolvelib provider | Lock strategies and marker/group population | Groups and marker propagation. |
| Conda | Environment package solve graph | SAT/libmamba over MatchSpec/PackageRecord | Repodata reduction, installed prefix state | Separate MatchSpec from selected PackageRecord. |
| Maven Resolver | Dependency descriptor graph | Collection plus conflict graph transformers | Repository session/cache | Scopes, dependency management, conflict metadata. |
| Gradle | Configuration/variant dependency graph | Dynamic graph traversal with conflicts/capabilities | Build cache/configuration cache | Source sets, variants, capabilities, dynamic unsupported tier. |
| Bazel | Target/configured target/Skyframe graph | Incremental key/value evaluation | Skyframe dependency graph | Registered dependency reads and target/configuration separation. |
| Pants | Target/source ownership/inference graph | Rule engine | Native engine and rule cache | Source ownership and inferred-vs-explicit dependency separation. |
| Nx | Project graph and external nodes | Plugin-created dependencies | File map/project graph cache | Partial graph errors, plugin edges, cache inputs. |
| Turborepo | Package graph and task graph | Package-manager graph plus task DAG | Lockfile/package discovery cache | Keep package graph and task graph separate. |
| Cargo | Package/workspace/feature graph | Backtracking/unification resolver | Shared lockfile and registry cache | Stable package identity, feature/target-conditioned edges. |

## Key Patterns To Reuse

- Typed node identity instead of path-only identity.
- Declared requirements separate from selected package instances.
- Workspaces as roots with explicit members and excludes.
- Lockfile schema/version preservation.
- Provider plugins or extension layers for graph creation.
- Cache keys over config/manifest/lockfile inputs.
- Partial graph with diagnostics instead of all-or-nothing failure.

## Key Patterns To Avoid

- Reusing install-tree layout as source import truth.
- Flattening conditions and variants.
- Treating build graph exactness as static parse output.
- Hiding conflicts or dynamic unsupported behavior.
- Making package manager execution mandatory for every scan.
