# Rust Cargo Package Graph Report

## Scope

Rust is not a primary polint target today, but Cargo is a valuable reference and should be easy to add later because polint itself is Rust.

Cargo support would cover:

- `Cargo.toml`;
- `Cargo.lock`;
- workspaces;
- path dependencies;
- features;
- target-specific dependencies;
- `[patch]` and `[replace]`.

## State Of The Art

Cargo has strong, documented semantics:

- A workspace is a collection of packages managed together.
- Workspaces share one `Cargo.lock` and output directory.
- The resolver selects versions from dependency requirements and records the result in `Cargo.lock`.
- The resolver uses constraints and heuristics, including version unification where possible.
- Feature resolver behavior is a major part of the selected graph.

The Cargo Book includes pseudo-code approximating dependency resolution: pick a dependency, try to unify with an already selected version, enumerate candidate versions, register a version, enqueue its dependencies, and backtrack on failure.

## OSS Implementation Evidence

Local repository:

- `repos/cargo`: Cargo implementation.

Key implementation areas:

- workspace loading and manifest parsing;
- resolver;
- lockfile handling;
- feature resolution;
- package ID/source identity.

## Algorithm

```python
def cargo_workspace(root):
    manifest = parse_toml(root / "Cargo.toml")
    members = expand_members(manifest.workspace.members, manifest.workspace.exclude)
    return [parse_manifest(member / "Cargo.toml") for member in members]

def cargo_resolve(workspace, registry):
    queue = initial_dependencies(workspace)
    selected = {}
    while queue:
        dep = pick_next_dep(queue)
        if can_unify(dep, selected):
            continue
        for candidate in candidate_versions(dep, registry):
            if compatible(candidate, selected):
                selected.add(candidate)
                queue.extend(candidate.dependencies)
                break
        else:
            backtrack()
    return selected
```

## Accuracy

High for:

- workspaces;
- static dependency declarations;
- path dependencies;
- lockfile-selected versions;
- feature edges when resolver version and target conditions are modeled.

Lower or conditional for:

- target-specific dependencies without selected target triple;
- build scripts generating code;
- proc macro effects on semantic index/call graph;
- registry metadata unavailable when no lockfile exists.

## Complexity

- Workspace discovery: `O(manifests + glob matches)`.
- Manifest parse: `O(total Cargo.toml bytes)`.
- Lockfile parse: `O(packages + edges)`.
- Resolve: backtracking worst case, practical with heuristics and unification.
- Feature propagation: graph/fixpoint over package-feature edges.

## Polint Implication

Cargo is a good reference for:

- stable package identity;
- workspace inheritance;
- lockfile exactness;
- feature/condition-aware dependency edges;
- dependency resolver docs that honestly describe heuristic/backtracking behavior.

Add Cargo support later as:

- a Rust provider;
- a validation target for polint's own repo;
- a reference for feature-conditioned edge representation.
