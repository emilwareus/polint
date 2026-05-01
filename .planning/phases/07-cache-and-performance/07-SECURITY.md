---
phase: 07
slug: cache-and-performance
status: verified
threats_open: 0
asvs_level: 1
created: 2026-05-01
verified: 2026-05-01T08:22:30Z
---

# Phase 07 — Security

Per-phase security contract: threat register, accepted risks, and audit trail.

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| CLI -> `.polint/cache` | `polint check` and `profile-rules` read/write local cache files. | Local parser diagnostics and extracted fact metadata. |
| parser workers -> shared analysis DB | Rayon workers parse per-file inputs and return facts for sequential merge. | Source-derived fact records and parser diagnostics. |
| rule execution -> profiling output | `profile-rules` prints per-rule timing rows. | Rule IDs, elapsed local timing, diagnostic counts. |
| cache state -> diagnostics | Cold, warm, and disabled cache states must not alter user-visible diagnostics. | JSON/human/SARIF diagnostic output. |

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-07-01-01 | Tampering | `.polint/cache/*.json` | mitigate | `Cache::read_json_or_miss` treats malformed cache files as misses; invalid JSON test passed. | closed |
| T-07-01-02 | Information Disclosure | cache values | mitigate | Cache payloads are `CachedFileAnalysis` DTOs with diagnostics and fact metadata; full source text is not stored and source-free tests passed. | closed |
| T-07-01-03 | Denial of Service | cache key collisions | mitigate | Cache stable IDs include relative path/content hash, config hash, rule hash, cache version, and schema; key invariant tests passed. | closed |
| T-07-01-04 | Repudiation | disabled cache behavior | mitigate | Cache and CLI tests prove disabled writes do not create `.polint/cache`. | closed |
| T-07-02-01 | Tampering | cached fact JSON | mitigate | Cache misses on invalid JSON and fact restoration remaps IDs through `AnalysisDb` push APIs before use. | closed |
| T-07-02-02 | Information Disclosure | cache payloads | mitigate | `cached_file_analysis_does_not_include_source_text` passed; cache metadata tests assert fixture source snippets are not written. | closed |
| T-07-02-03 | Denial of Service | stale cache entries | mitigate | Go and TS cache keys include content/config/rule/version/schema inputs; cache tests prove invalidation dimensions. | closed |
| T-07-02-04 | Spoofing | parser diagnostics from cache | mitigate | Parser diagnostics are stored only under content-addressed language-schema keys and preserve `parser/go` and `parser/ts` identities. | closed |
| T-07-03-01 | Tampering | parallel fact merge | mitigate | Workers use local `AnalysisDb` values; results are sorted by `FileId` and restored sequentially. | closed |
| T-07-03-02 | Denial of Service | Rayon parsing | mitigate | Uses Rayon per-file parallel iterators and the existing global pool; no unbounded thread spawning was added. | closed |
| T-07-03-03 | Repudiation | nondeterministic diagnostics | mitigate | Core, adapter, and CLI repeated-run determinism tests passed. | closed |
| T-07-03-04 | Information Disclosure | worker payloads | accept | Parallel workers only process local source already loaded for analysis and do not introduce a new external sink. | closed |
| T-07-04-01 | Repudiation | `profile-rules` rows | mitigate | Integration tests prove deterministic row ordering and diagnostic-count fields. | closed |
| T-07-04-02 | Information Disclosure | timing output | accept | Rule IDs and timing durations are local CLI metadata; they do not expose source text. | closed |
| T-07-04-03 | Denial of Service | profiling run | mitigate | Profiling reuses single analysis setup and times each rule once; no benchmark loops or unbounded work were added. | closed |
| T-07-04-04 | Spoofing | cache warm versus cold output | mitigate | Cold/warm/no-cache exact JSON equality tests passed and prove cache state does not spoof diagnostics. | closed |

Status legend: `closed` threats have either implemented mitigation evidence or an explicit accepted-risk entry.

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-07-01 | T-07-03-04 | Rayon workers handle only local source already loaded for analysis and write no new external output. | Project owner via Phase 7 threat model | 2026-05-01 |
| AR-07-02 | T-07-04-02 | `profile-rules` timing output exposes local rule IDs, elapsed time, and diagnostic counts only. | Project owner via Phase 7 threat model | 2026-05-01 |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-05-01 | 16 | 16 | 0 | Codex |

## Verification Evidence

- `cargo test -p polint-cache --lib` passed cache key, disabled-cache, invalid JSON, and property tests.
- `cargo test -p polint-cli --test cli cache` passed cold/warm/no-cache and repeated-run determinism tests.
- `cargo test -p polint-core --lib run_rules_parallel_matches_sequential` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed.

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-05-01
