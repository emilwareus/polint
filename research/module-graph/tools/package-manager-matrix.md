# Package Manager Matrix

This matrix summarizes what polint should support for popular package managers.

## Priority

| Priority | Meaning |
|---|---|
| P0 | Required for first useful module graph. |
| P1 | Required for high-value multi-language support. |
| P2 | Important after foundations. |
| P3 | Later or extension-first. |

## Matrix

| Ecosystem | Manager | Priority | Native Inputs | First Polint Support | Precision Ceiling |
|---|---|---:|---|---|---|
| Go | Go modules | P0 | `go.mod`, `go.work`, `go.sum`, vendor | Full native parse and MVS tier | High |
| TS/JS | npm | P0 | `package.json`, `package-lock.json`, workspaces | Manifest/workspace/lockfile reader | High with lockfile |
| TS/JS | pnpm | P0 | `pnpm-workspace.yaml`, `pnpm-lock.yaml`, `.npmrc` | Workspace protocol and lockfile reader | High with peer context |
| TS/JS | Yarn v1 | P1 | `package.json`, `yarn.lock` | Workspace and lockfile reader | High with caveats |
| TS/JS | Yarn Berry/PnP | P1 | `.yarnrc.yml`, `yarn.lock`, `.pnp.cjs` | Detect and parse/map PnP facts | High if PnP map supported |
| TS/JS | Bun | P1 | `package.json`, `bun.lock`, Bun config | Workspace/lockfile recognition, text lock parser first | Medium to high |
| Python | pip | P0 | requirements files, pyproject dependencies | PEP 508 and requirements parser | Declared high, resolved lower without lock |
| Python | uv | P0 | `pyproject.toml`, `uv.lock`, `[tool.uv.workspace]` | Workspace and lockfile reader | High |
| Python | Poetry | P1 | `pyproject.toml`, `poetry.lock` | Manifest and lockfile reader | High |
| Python | PDM | P1 | `pyproject.toml`, `pdm.lock` | Manifest and lockfile reader | High |
| Python | conda | P2 | `environment.yml`, conda-lock, channel metadata | Environment manifest first | Medium without solver/channel metadata |
| Java/JVM | Maven | P0 | `pom.xml` | Native static provider | High for static Maven |
| Java/JVM | Gradle | P1 | settings/build files, lockfiles | Project/source-set static tier | Medium unless tool-reported/extension |
| Java/JVM | Bazel | P2 | `MODULE.bazel`, `WORKSPACE`, `BUILD` | Conservative target facts | Medium unless configured graph available |
| Java/JVM/multi | Pants | P2 | `pants.toml`, BUILD files | Conservative target/source-owner facts | Medium unless Pants engine facts available |
| Rust | Cargo | P2 | `Cargo.toml`, `Cargo.lock` | Good later target and internal validation | High |

## Detection Rules

```python
def detect_manager(root):
    signals = []
    signals += package_manager_field(root / "package.json")
    signals += lockfile_signals(root)
    signals += workspace_config_signals(root)
    signals += language_manifest_signals(root)
    return choose_highest_confidence(signals)
```

Recommended precedence for TS/JS:

1. `package.json#packageManager`.
2. Manager-specific workspace file: `pnpm-workspace.yaml`, `.yarnrc.yml`, Bun config.
3. Lockfile: `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `package-lock.json`.
4. Fallback to npm-compatible.

Recommended precedence for Python:

1. Lockfile: `uv.lock`, `poetry.lock`, `pdm.lock`.
2. Tool sections in `pyproject.toml`.
3. Requirements files.
4. Virtualenv/site-packages hints as environment facts, not package-manager root facts.

Recommended precedence for Java/JVM:

1. Build root files: `pom.xml`, `settings.gradle*`, `MODULE.bazel`, `pants.toml`.
2. Multi-module/project declarations.
3. Standard source layout fallback.

## First Native Parser Targets

| Format | Parser Type | Notes |
|---|---|---|
| JSON package files | structured JSON | Preserve spans for diagnostics where possible. |
| TOML pyproject/Cargo | structured TOML | Use existing TOML parser in repo stack. |
| YAML pnpm lock/workspace | structured YAML or limited parser | Add dependency only if acceptable; otherwise strict subset parser with diagnostics. |
| XML Maven POM | structured XML | Need namespaces, parents, dependency management. |
| Gradle | static recognizer, not full parser | Honest conservative facts. |
| Bazel/Pants BUILD | static recognizer later | Avoid pretending full Starlark/Pants evaluation. |

## Required Edge Dimensions

Do not store just `from -> to`. Store:

- source manager;
- declared vs resolved;
- direct vs transitive;
- version/range;
- selected version;
- scope/kind;
- optional/peer/dev/test/build/runtime;
- conditions/markers/platform;
- source span;
- lockfile key/context;
- precision.

## Why This Matters For Rules

Rules should be able to ask:

- "Does package A declare dependency B?"
- "Does package A actually import B?"
- "Is B a dev/test dependency only?"
- "Is this edge exact, lockfile exact, extension-provided, or heuristic?"
- "Which package manager created this edge?"
- "Does this dependency cross an architecture layer?"
- "Is this dependency only active on Python 3.12/Linux/test source set?"

Those questions require normalized facts with manager-specific details preserved.
