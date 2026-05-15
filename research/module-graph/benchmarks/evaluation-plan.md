# Module Graph Evaluation Plan

## What To Measure

| Metric | Definition |
|---|---|
| Root precision/recall | Did we find the same workspace roots as the package/build tool? |
| Package precision/recall | Did we find the right workspace packages/projects/modules? |
| Declared edge precision/recall | Did parsed manifest requirements match tool output? |
| Resolved edge precision/recall | Did lockfile/native/tool-reported selected dependencies match? |
| Import ownership accuracy | Did source imports resolve to the expected package/module? |
| Source-set accuracy | Did main/test/generated/tool source sets match build tool expectations? |
| Unknown quality | Are dynamic/missing/unsupported facts specific and actionable? |
| Runtime/memory | Cost over repository size and lockfile size. |
| Cache invalidation | Does changing a manifest/lockfile/config invalidate exactly the affected layer? |
| Extension delta | How much do repo-local providers improve exactness and reduce unknowns? |

## External Oracles

Use external tools as validation oracles, not runtime dependencies:

- Go: `go list -m all`, `go list ./...`, `go env GOWORK`.
- npm: `npm ls --all --json`, Arborist fixtures.
- pnpm: `pnpm list --recursive --json`, lockfile fixtures.
- Yarn: `yarn workspaces list --json`, PnP API fixtures.
- Bun: `bun pm ls` or lockfile fixture comparison where available.
- Python/pip: `pip inspect`, `pipdeptree`, resolvelib fixtures.
- uv: `uv tree`, `uv lock` fixtures.
- Poetry: `poetry show --tree`.
- PDM: `pdm list --graph`.
- Conda: `conda list --json`, `conda repoquery`, `conda-lock` fixtures.
- Maven: `mvn dependency:tree`, Maven Resolver tests.
- Gradle: `gradle dependencies`, `outgoingVariants`, dependency insight fixtures.
- Bazel: `bazel query`, `bazel cquery`.
- Pants: `pants dependencies`, `pants peek`.
- Nx/Turborepo: project graph/task graph outputs.

## Native Fixture Schema

```toml
[[expected.packages]]
key = "js:npm:@repo/ui@1.0.0"
path = "packages/ui"
manager = "pnpm"
precision = "ManifestExact"

[[expected.requirements]]
from = "js:npm:@repo/app@1.0.0"
name = "@repo/ui"
kind = "runtime"
requested = "workspace:*"

[[expected.resolved_edges]]
from = "js:npm:@repo/app@1.0.0"
to = "js:npm:@repo/ui@1.0.0"
source = "Lockfile"
status = "Exact"

[[expected.unknowns]]
kind = "UnsupportedDynamicBuildScript"
path = "build.gradle.kts"
```

## Fixture Families

### Go

- single module;
- multi-module `go.work`;
- local replace;
- vendor mode;
- build tags;
- tests;
- missing module metadata.

### TS/JS

- npm workspaces;
- pnpm workspace protocol;
- Yarn Berry PnP;
- Bun workspaces;
- tsconfig paths;
- package `exports`;
- undeclared hoisted dependency;
- peer dependency contexts.

### Python

- pyproject standard deps;
- optional deps and dependency groups;
- requirements include/constraints;
- uv workspace;
- Poetry/PDM locks;
- path/editable dependency;
- marker-conditioned deps.

### Java/JVM

- Maven single/multi-module;
- Maven dependency management/BOM/exclusions;
- Gradle standard source sets;
- Gradle dynamic unsupported;
- Bazel static target labels;
- Pants source ownership.

## Default-Vs-Extension Evaluation

For each fixture with repo-specific conventions:

```text
default run:
  unknown import alias count = N
  exact dependency edges = X

extension run:
  unknown import alias count = N - delta
  exact dependency edges = X + delta
  extension conflicts = 0
```

Extensions must improve measured accuracy without suppressing native unknowns silently.

## Acceptance Thresholds

Before public SDK:

- root/package/dependency fact fixtures pass deterministically;
- cache invalidation fixtures pass;
- extension merge conflict fixtures pass;
- at least one real-world repo per P0 ecosystem is scanned;
- generated diagnostics point to exact manifest/lockfile/config spans where possible;
- `git diff --check` and Markdown link sanity pass.
