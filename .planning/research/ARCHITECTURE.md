# Architecture Research: polint

## Component Boundaries

- `polint-cli`: owns command parsing, terminal UX, exit codes, and orchestration.
- `polint-config`: owns `.polint.toml` loading, defaults, profiles, rule config, and glob settings.
- `polint-fs`: owns repository walking, ignore handling, language detection by path, and deterministic file ordering.
- `polint-diagnostics`: owns diagnostic types, severities, labels, fixes, evidence, fingerprints, and output renderers.
- `polint-core`: owns analysis database, source files, stable IDs, fact storage, adapter coordination, rule registry, rule runner, deduplication, and deterministic sorting.
- `polint-sdk`: public rule-author API. It should wrap core capabilities so rule authors do not need internal parser details.
- `polint-go`: Go parser and extractor using tree-sitter-go.
- `polint-ts`: TS/JS parser and extractor using Oxc.
- `polint-graph`: internal relationship helpers around petgraph; not a public CLI or SDK boundary.
- `polint-rules`: built-in example rules using the SDK.
- `polint-cache`: content/config/rule hashing and parse/fact cache metadata.

## Data Flow

1. CLI loads config.
2. Config resolves profile, rule settings, includes, excludes, severity overrides, and language settings.
3. File discovery returns deterministic `SourceFile` inputs.
4. Cache hashes source/config/rule inputs and answers whether cached facts are usable.
5. Core asks language adapters to parse and extract requested facts.
6. Rule registry chooses enabled rules for the profile.
7. Rule runner computes capability-gated facts and executes rules.
8. Diagnostics are collected, deduplicated, sorted, rendered, and converted into exit codes.

## Suggested Build Order

1. Skeleton workspace, diagnostics, config, filesystem, and CLI.
2. Core database, SDK trait, registry, runner, and default rules.
3. Go adapter and Go example rules.
4. TS adapter and TS example rules.
5. Caching, profiling, and SARIF.

## Integration Notes

- Keep `polint-sdk` free of CLI concerns.
- Avoid language-specific types leaking into generic SDK APIs unless they are deliberately namespaced.
- Store source text once per file; facts should hold IDs and spans, not cloned source chunks.
- Rule execution must be panic-contained where possible and report controlled internal rule errors.
