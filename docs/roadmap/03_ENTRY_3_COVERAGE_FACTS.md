# Entry 3: Coverage Facts

## Goal

Fulfill the `CoverageFacts<'_>` typed view by importing external coverage
reports and mapping them to polint source facts.

## Why

Coverage connects static analysis to CI evidence. It enables rules such as
"new error branches must have coverage evidence."

## Difficulty

**M** for line coverage, **L** for branch/function mapping, **XL** for exact
cross-language branch mapping.

## What To Build

- coverage config in `.polint.toml`
- `LineCoverageFact`
- `BranchCoverageFact`
- `FunctionCoverageFact`
- `CoverageSource`
- `CoveragePrecision`
- `CoverageFacts<'_>::for_file(file_id)`
- `CoverageFacts<'_>::for_function(function_id)`
- `CoverageFacts<'_>::for_branch(branch_id)`

## Build Method

1. Add config sections such as `[coverage.go]`, `[coverage.ts]`,
   `[coverage.python]`, and `[coverage.java]`.
2. Parse Go `coverprofile` files from `go test -coverprofile`.
3. Parse LCOV for TS/JS.
4. Later parse coverage.py XML/JSON for Python.
5. Later parse JaCoCo XML for Java.
6. Normalize report paths to repo-relative source files.
7. Map line intervals first.
8. Map function and branch coverage where stable spans exist.
9. Emit setup diagnostics when coverage is requested but reports are missing.

## Done When

- Go and TS/JS coverage reports produce public coverage facts.
- Rules can distinguish line, function, and branch precision.
- Missing reports create clear diagnostics.
- Cache keys include report paths and report content hashes.

## Later Languages

- Python should support coverage.py XML/JSON.
- Java should support JaCoCo XML.
