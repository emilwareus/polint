# Python Package Graph Report

## Scope

Python package graph support should cover:

- pip and requirements files;
- `pyproject.toml`;
- uv;
- Poetry;
- PDM;
- conda environment files later;
- PEP 508 requirements, extras, markers, groups;
- editable/path/URL/git dependencies;
- virtual environments and stubs as later import-resolution inputs.

## State Of The Art

Python dependency topology is environment-sensitive. A dependency can be conditional on Python version, platform, extra, dependency group, URL/source, editable mode, or installed prefix state.

The Python Packaging User Guide defines PEP 508 dependency specifiers: a distribution name, extras, version limits or URL, and environment markers. Dependency groups define named groups in `pyproject.toml` with include expansion and cycle detection.

pip uses resolvelib with a provider API. uv implements a modern Rust resolver using PubGrub and has explicit workspace discovery/cache keys. Poetry uses a Mixology/PubGrub-style solver with marker aggregation and override retries. PDM uses resolvelib and lock strategies. Conda solves richer environment specs with MatchSpec and PackageRecord objects, now using libmamba by default.

## Package Managers To Support

| Manager | Must Support | Notes |
|---|---|---|
| pip | `requirements*.txt`, constraints, PEP 508 requirements, installed environment later | Most common baseline. |
| uv | `pyproject.toml`, `uv.lock`, `[tool.uv.workspace]`, dependency groups, sources | Modern Rust implementation and strong workspace model. |
| Poetry | `pyproject.toml`, `poetry.lock`, groups/extras/markers | Very common application/library manager. |
| PDM | `pyproject.toml`, `pdm.lock`, groups/strategies | PEP 582/pyproject-centric manager. |
| conda | `environment.yml`, `conda-lock`, MatchSpec/channel metadata later | Important for data science/ML; solver exactness is environment/channel dependent. |

## OSS Implementation Evidence

Local repositories:

- `repos/pip`: pip's resolvelib adapter.
- `repos/resolvelib`: generic backtracking resolver.
- `repos/uv`: Rust workspace and resolver internals.
- `repos/poetry`: Mixology/PubGrub-style solving and lock handling.
- `repos/pdm`: resolvelib-based lock resolver.
- `repos/conda`: MatchSpec/PackageRecord and solver deep-dive docs/code.
- `repos/ty` and `repos/pyright`: import resolution and environment/stub semantics, relevant for later import-to-package resolution.

Key files:

- `resolvelib/src/resolvelib/resolvers/resolution.py`
- `pip/src/pip/_internal/resolution/resolvelib/resolver.py`
- `uv/crates/uv-workspace/src/workspace.rs`
- `uv/crates/uv-resolver/src/resolver/mod.rs`
- `uv/crates/uv-resolver/src/pubgrub/`
- `poetry/src/poetry/puzzle/solver.py`
- `pdm/src/pdm/resolver/resolvelib.py`
- `conda/docs/source/dev-guide/deep-dives/solvers.md`
- `conda/conda/resolve.py`

## Algorithm

```python
def parse_python_requirements(root):
    pyproject = parse_pyproject_if_exists(root)
    reqs = []
    if pyproject.project:
        reqs += parse_pep508_list(pyproject.project.dependencies, group="main")
        reqs += parse_optional_dependencies(pyproject.project.optional_dependencies)
        reqs += parse_dependency_groups(pyproject.dependency_groups)
    reqs += parse_requirements_files(root)
    reqs += parse_tool_specific_sections(pyproject.tool)
    return reqs

def edge_applies(requirement, environment):
    if requirement.marker is None:
        return True
    return evaluate_marker(requirement.marker, environment)

def build_python_declared_graph(packages, environment):
    for pkg in packages:
        for req in pkg.requirements:
            emit_requirement(pkg, req)
            if edge_applies(req, environment):
                emit_conditional_candidate(pkg, req)
```

Lockfile path:

```python
def parse_python_lockfiles(root):
    if exists("uv.lock"):
        return parse_uv_lock()
    if exists("poetry.lock"):
        return parse_poetry_lock()
    if exists("pdm.lock"):
        return parse_pdm_lock()
    return []
```

## Accuracy

High for:

- PEP 508 requirement parsing;
- static `pyproject.toml` dependencies;
- dependency groups and optional dependencies;
- committed lockfile selected versions;
- local path/editable dependency discovery.

Lower or conditional for:

- dynamic build metadata;
- setup.py side effects;
- package metadata that requires building an sdist;
- installed environment differences;
- namespace packages;
- `.pth` files;
- conda channel solves;
- platform marker universes not selected by config.

## Complexity

- Pyproject/requirements parse: `O(total input bytes)`.
- Dependency group expansion: `O(groups + include edges)`, with cycle detection.
- Lockfile parse: `O(lockfile entries + edges)`.
- resolvelib/pip-style solve: exponential worst case, practical backtracking with round limits.
- PubGrub/uv-style solve: conflict-learning dependency solving, still worst-case hard but better explanations and pruning.
- Conda SAT solve: worst-case exponential, cost dominated by channel metadata reduction and SAT search.

## Polint Implementation

Implement first:

- PEP 508 parser or strict subset with diagnostics;
- dependency group expansion;
- `pyproject.toml` standard fields;
- requirements file subset;
- uv/Poetry/PDM lockfile readers;
- path/editable local package detection;
- environment marker representation.

Defer:

- full PyPI solving;
- building sdists to get metadata;
- exact conda solving;
- execution of setup.py.

## Precision Labels

- `ManifestExact` for PEP 508/static pyproject requirements.
- `LockfileExact` for uv/Poetry/PDM lockfile edges.
- `PlatformConditional` for marker-dependent edges.
- `Dynamic` for build metadata not available statically.
- `ExtensionValidated` for repo-specific packaging models.

## First Fixtures

- `pyproject.toml` with main, optional, and dependency-group dependencies.
- Requirements file with `-r` include and constraints.
- Path/editable local package.
- uv workspace with two packages.
- Poetry lock with extras/markers.
- PDM lock with groups.
- Dynamic setup.py unsupported fixture.
