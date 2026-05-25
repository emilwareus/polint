# Phase 39 Pattern Map: Slicing, Paths, and Evidence Bundles

## Closest Existing Patterns

| Planned Area | Closest Analog | Pattern To Reuse |
|---|---|---|
| Evidence fact contracts and store | `crates/polint/src/analysis/data_flow/{facts,store}.rs` | Private row structs with stable keys, run-local dense ids, normalized output, rebuilt indexes, metadata refresh, and store-level reference validation. |
| Provider/cache wiring | `crates/polint/src/analysis/data_flow/{provider,cache_key}.rs`, `crates/polint/src/analysis_kernel/provider.rs` | Provider manifest entry after upstream facts, deterministic parameter/input/output digests, absent sentinels, and output replacement through `AnalysisDb`. |
| Debug rows | `crates/polint/src/analysis/data_flow/debug.rs`, `crates/polint/src/analysis_kernel/debug.rs` | Test-only debug JSON with compact rows/counts, relative paths only, no raw source, no timestamps, and deterministic maps. |
| Validation | `crates/polint/src/analysis/data_flow/validate.rs`, `crates/polint/src/analysis_kernel/validation.rs` | Local validator returning structured issues, then kernel validation renders internal diagnostics with family/stable-key/reason evidence. |
| Eval observation | `crates/polint/src/eval/{model,observed,fixtures}.rs` | Add area/fact-family vocabulary, parse debug JSON into `ObservedItem`s, add taxonomy fixture checks and non-zero focused test filters. |
| Public no-leak proof | `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/tests/cli.rs` | Scan public CLI JSON/help, SDK, runner, README, and docs/facts for private markers while allowing intentionally reserved SDK view names. |
| Report rendering | `crates/polint/src/reporting.rs`, `crates/polint/src/cli/mod.rs`, `crates/polint/src/runner/mod.rs` | Preserve existing diagnostic JSON/SARIF/human output shape; add structured evidence in a deterministic, bounded way and keep scalar evidence compatibility. |

## Implementation Guidance

- Keep all new evidence and slicing internals `pub(crate)` unless they are existing public diagnostic/report fields.
- Stable keys must come from source fact stable keys and query configuration, not dense ids.
- Use compact labels and stable references instead of raw source bodies, AST dumps, parser object ids, or absolute paths.
- Treat unknown, unsupported, setup-missing, rejected, omitted, and budget-exceeded as visible rows.
- Prefer query-specific evidence graph views over eagerly storing all possible paths.
- JSON is the primary structured renderer; SARIF is a lossy projection from the same evidence model.

## Candidate Files

- `crates/polint/src/analysis/evidence/{facts,store,provider,cache_key,query,rank,render,validate,debug}.rs`
- `crates/polint/src/analysis/slicing/{mod,local,paths,interprocedural}.rs`
- `crates/polint/src/analysis/mod.rs`
- `crates/polint/src/core/mod.rs`
- `crates/polint/src/analysis_kernel/{provider,debug,validation}.rs`
- `crates/polint/src/diagnostics/mod.rs`
- `crates/polint/src/reporting.rs`
- `crates/polint/src/cli/mod.rs`
- `crates/polint/src/runner/mod.rs`
- `crates/polint/src/eval/{model,observed,fixtures}.rs`
- `tests/eval-fixtures/evidence/`
