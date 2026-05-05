# Stream B — Go / TypeScript adapters — Rust best-practices audit

**Scope:** All Rust under `crates/polint/src/go/` and `crates/polint/src/ts/` (each tree is a single `mod.rs`: ~2.4k and ~4.0k lines respectively).

**Reference:** Project skill `.agents/skills/rust-best-practices/SKILL.md` (Apollo GraphQL Rust Best Practices Handbook): borrowing/clones, error handling (`Result`, avoid panics in production), performance hygiene, thread safety (`Send`/`Sync`), linting expectations.

## Summary

Both adapters follow the same orchestration pattern: filter files → optional `rayon` parallel per-file work → merge facts/diagnostics; parse failures become `parser/go` or `parser/ts` diagnostics instead of aborting the run. **TypeScript** parsing uses a **fresh** `oxc_allocator::Allocator` and `oxc_parser::Parser` per file, which is **sound under `par_iter`**. **Go** uses a **`thread_local` `RefCell<Option<Parser>>`**, which gives each OS thread its own `tree_sitter::Parser` and avoids sharing non-`Sync` parser state across rayon workers—an appropriate pattern for this stack.

The main **handbook mismatch** in production Go code is a single **`unwrap()`** on the parser slot after initialization. The handbook also discourages **`anyhow::Result` in library-style APIs**; both adapters propagate `anyhow` from parsing helpers—consistent with the current crate, but not ideal per Apollo Ch. 4.4 if you ever tighten public error types.

**`#[cfg(test)]`:** Test entrypoints (`analyze`, `analyze_with_cache`) are correctly gated; integration-style tests live in the same files. `unwrap`/`expect` in those tests and helpers are acceptable per the skill.

There is **no `unsafe` Rust**, no `#[allow(dead_code)]` / `#[allow(clippy::…)]` in these modules, and no Rust `unsafe` beyond fixture strings in tests.

---

## Findings (by severity)

### Medium — production `unwrap` after guaranteed `Some`

- **Where:** `crates/polint/src/go/mod.rs` — `parse_go_file`, inside `GO_PARSER.with(|cell| -> Result<_> { ... })`, approximately the `slot.as_mut().unwrap()` after filling `slot` when `None`.
- **Why:** Apollo Ch. 4.2: avoid `unwrap`/`expect` outside tests even when “obviously” safe; use `let`-`else`, `?`, or an internal error that becomes a diagnostic if the invariant breaks.
- **Suggested direction:** Bind with `let Some(parser) = slot.as_mut() else { return Err(...).context(...)); }` or map to a single `anyhow` context string; avoids a latent panic path if tree-sitter init logic changes.

### Low — `anyhow::Result` in adapter / parse helpers (library context)

- **Where:** `crates/polint/src/go/mod.rs` (`parse_go_file` → `Result<Vec<Diagnostic>>`), `crates/polint/src/ts/mod.rs` (`parse_ts_file` → `Result<Vec<Diagnostic>>`).
- **Why:** Handbook recommends `thiserror` for crate/library surfaces and reserving `anyhow` for binaries (Ch. 4.3–4.4). Here `anyhow` is ergonomic and errors are converted to diagnostics at the boundary; acceptable as project policy, but not “textbook” for a reusable library façade.

### Low — clone / allocation hot spots (review if profiling says so)

- **Where (Go):** `crates/polint/src/go/mod.rs` — `analyze_go_source_file` clones `diagnostics` and `facts` into `CachedFileAnalysis` while also returning owned facts (by design for cache + DB restore). `push_if_branches` clones `condition` more than once when pushing two branch edges.
- **Where (TS):** `crates/polint/src/ts/mod.rs` — same cache pattern as Go; `template_literal_value` builds a `Vec` then `join("")` (`collect` + allocate); many `to_string()` calls on AST atoms during walks (expected for fact extraction).
- **Why:** Ch. 3 “avoid redundant clones”; skill flags `redundant_clone` / needless `collect`. None of these are clearly wrong without benchmarks—they are **candidates** if adapters show up in profiles.

### Low — minor redundancy / style

- **Where:** `crates/polint/src/go/mod.rs` — `import_alias`: `unwrap_or_else` repeats `source.get(spec.start_byte()..path.start_byte())` already available in the outer `?` chain.
- **Why:** Small avoidable work; readability/maintainability.

### Info — module size and structure

- **Where:** `crates/polint/src/ts/mod.rs` (~3.2k lines before `mod tests`), `crates/polint/src/go/mod.rs` (~1.3k lines before tests).
- **Why:** Large single files with many mutually recursive AST walkers (`collect_require_*`, `walk_*_for_literals`, complexity helpers). Not a correctness issue; affects reviewability and merge conflict risk. Apollo doesn’t forbid this, but splitting into `go/parser.rs`, `go/branches.rs`, `ts/require.rs`, `ts/literals.rs`, etc. would match common Rust module hygiene.

### Info — tests vs production API surface

- **Where:** Both modules: `pub fn analyze_with_options(...)` is the real entry; `analyze` / `analyze_with_cache` are `#[cfg(test)]` only.
- **Why:** Clear separation; aligns with Ch. 5’s emphasis on test helpers living next to code without widening the stable API.

### Positive — parallel safety

- **Go:** `thread_local!` + `RefCell` for `tree_sitter::Parser` under `par_iter` — avoids `Sync` requirements on the parser type; each worker thread gets its own instance.
- **TS:** Per-file `Allocator` + `Parser` — no shared mutable parser state across threads.

### Positive — TS production path and panics

- **Where:** `crates/polint/src/ts/mod.rs` (production section before `#[cfg(test)] mod tests`).
- **Why:** No `.unwrap()` / `.expect()` on `Result`/`Option` in production Rust; parser errors are surfaced as diagnostics; `unwrap_or` / `unwrap_or_else` / `unwrap_or_default` used for defaults only.

---

## File index (audited)

| Path | Role |
|------|------|
| `crates/polint/src/go/mod.rs` | Go facts extraction, cache, branch heuristics, tests |
| `crates/polint/src/ts/mod.rs` | TS/JS facts extraction (Oxc), JSX, CommonJS `require`, tests |

---

_End of Stream B adapter audit._
