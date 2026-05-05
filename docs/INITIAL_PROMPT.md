You are an expert Rust systems engineer and static-analysis/linter architect. Build a high-performance, extensible static-analysis framework in Rust whose primary purpose is to make it extremely easy for users to write their own repo-local linters.

This is not primarily a packaged ruleset.

This is not meant to replace existing linters such as ESLint, Ruff, Biome, golangci-lint, or language-specific formatters.

The goal is to build a framework for creating custom, codebase-specific static-analysis rules in an AI-native development world.

The core idea:

Modern AI coding agents are good at generating code, but they often fail to consistently follow project-specific conventions, architecture boundaries, test quality expectations, and subtle engineering patterns. Repeating those expectations in prompts is unreliable. Encoding them as executable repo-local static-analysis rules is much more reliable.

This tool should make that easy.

The framework should provide the hard infrastructure:

```text
file discovery
parsing
AST/CST access
symbols where available
imports
control-flow graphs
call graphs
branch obligations
test facts
coverage facts
caching
parallel execution
diagnostics
rule testing
CI output
```

Users should write the policy logic.

A rule is just code.

A rule should be a small program that consumes analysis facts and emits diagnostics.

The first-class user experience should be:

```text
“I want to enforce this local engineering policy in my repo.”

→ write a small rule inside the repo
→ run the tool
→ get precise diagnostics
→ use the same diagnostics locally, in CI, and with AI coding agents
```

The framework should initially support Go and TypeScript/JavaScript. Design it so more languages can be added later.

Use Rust.

Use the latest stable Rust edition available. At the time of writing, use Rust 2024 edition where compatible.

Before implementing, verify the latest crate versions with `cargo search` or crates.io, then use the latest compatible stable versions. As of 2026-04-28, start from this dependency baseline, updating if newer compatible versions exist:

Core dependencies:

* `clap = "4.6.1"` with derive support for CLI parsing.
* `serde = "1.0.228"` with derive.
* `serde_json = "1.0"` latest compatible.
* `toml = "0.9.8"` for config parsing.
* `anyhow = "1.0"` latest compatible for application errors.
* `thiserror = "2.0"` latest compatible for typed library errors.
* `tracing = "0.1"` latest compatible.
* `tracing-subscriber = "0.3"` latest compatible.
* `rayon = "1.11.0"` or latest compatible for parallel execution.
* `ignore = "0.4.25"` for fast repo walking with `.gitignore` support.
* `globset = "0.4"` latest compatible for file matching.
* `walkdir = "2"` only if needed; prefer `ignore`.
* `petgraph = "0.8.3"` for import graphs, call graphs, CFGs, and DOT export.
* `salsa = "0.26.1"` or latest compatible for incremental computation, if practical. If Salsa integration slows delivery, implement a simpler hash-based cache first and leave Salsa behind an abstraction.

Go parsing:

* `tree-sitter = "0.26.8"`.
* `tree-sitter-go = "0.25.0"`.

TypeScript/JavaScript parsing:

* Prefer Oxc crates, because Oxc is Rust-native and designed for high-performance JS/TS tooling.
* `oxc_parser = "0.123.0"` or latest compatible.
* `oxc_ast = "0.123.0"` or matching latest compatible.
* `oxc_span = "0.123.0"` or matching latest compatible.
* `oxc_allocator = "0.123.0"` or matching latest compatible.
* `oxc_semantic = "0.123.0"` if useful for semantic information.
* `oxc_resolver = "11"` or latest compatible if import resolution is needed.

Testing:

* `insta = "1"` latest compatible for snapshot tests.
* `assert_cmd = "2"` latest compatible for CLI integration tests.
* `predicates = "3"` latest compatible.
* `tempfile = "3"` latest compatible.
* `pretty_assertions = "1"` latest compatible.
* `proptest = "1"` latest compatible for parser/cache/rule invariants where useful.

Suggested project name: `polint`.

Create a Rust workspace with this structure:

```text id="ilpdry"
polint/
  Cargo.toml
  crates/
    polint-cli/
    polint-core/
    polint-config/
    polint-diagnostics/
    polint-fs/
    polint-cache/
    polint-sdk/
    polint-go/
    polint-ts/
    polint-graph/
    polint-rules/
  tests/
    fixtures/
      go/
      ts/
      mixed/
    snapshots/
  examples/
    basic/
    custom-rule-go/
    custom-rule-ts/
    go-branch-obligations/
    ts-design-tokens/
```

Crate responsibilities:

`polint-cli`

* Owns the binary.
* Provides commands:

  * `polint init`
  * `polint check`
  * `polint check --profile fast`
  * `polint check --profile full`
  * `polint check --format human`
  * `polint check --format json`
  * `polint check --format sarif`
  * `polint explain <rule-id>`
  * `polint new-rule <rule-name>`
  * `polint test-rules`
  * `polint profile-rules`
  * `polint graph imports --format dot`
  * `polint graph function <function-name> --format dot`
* Must have good help text and examples.
* The UX should emphasize custom rules, not built-in rule packs.

`polint-config`

* Loads `.polint.toml`.
* Supports rule configuration, profiles, include/exclude globs, severity overrides, and language settings.
* Supports discovering repo-local rules from `.polint/rules`.
* Config should be stable and easy to understand.
* Config should not feel like the primary way to define all policy. The primary power is custom code rules.

`polint-fs`

* Discovers files.
* Respects `.gitignore`.
* Supports include/exclude globs.
* Detects Go, TypeScript, JavaScript, TSX, JSX.

`polint-cache`

* Hashes file contents and config.
* Caches parse results/facts where possible.
* Must be safe to disable.
* CLI flag: `--no-cache`.
* Cache dir default: `.polint/cache`.

`polint-core`

* Owns the analysis database and rule runner.
* Defines stable IDs:

  * `FileId`
  * `NodeId`
  * `FunctionId`
  * `PackageId`
  * `BranchId`
  * `ImportId`
  * `RuleId`
* Defines core models:

  * `SourceFile`
  * `Span`
  * `TextRange`
  * `FunctionFact`
  * `ImportFact`
  * `BranchObligation`
  * `TestFact`
  * `CoverageFact`
  * `AnalysisDb`
* Coordinates language adapters and analyzers.
* Runs rules in parallel when safe.
* Deduplicates diagnostics.
* Sorts diagnostics deterministically.

`polint-diagnostics`

* Defines:

  * `Diagnostic`
  * `Severity`
  * `Label`
  * `Suggestion`
  * `Fix`
  * `Evidence`
  * `DiagnosticCode`
* Renders:

  * human terminal output
  * JSON
  * SARIF-like output
* Include stable fingerprints for diagnostics.
* Diagnostics should be optimized for both humans and AI agents.

`polint-sdk`

* Public API for custom rules.
* This is one of the most important crates.
* Keep this clean, ergonomic, stable, and well documented.
* Rule authors should not need to know internal parser details.
* Built-in example rules should use the same SDK that users use.
* Expose a trait like:

```rust id="r4zrr8"
pub trait Rule: Send + Sync {
    fn meta(&self) -> RuleMeta;
    fn capabilities(&self) -> Capabilities;
    fn run(&self, ctx: &mut RuleCtx) -> Result<()>;
}
```

* `RuleCtx` should expose high-level queries:

  * `ctx.files().matching("internal/**/*.go")`
  * `ctx.functions().matching("internal/domain/**/*.go")`
  * `ctx.import_graph()`
  * `ctx.call_graph()`
  * `ctx.cfg(function)`
  * `ctx.cyclomatic_complexity(function)`
  * `ctx.branch_obligations(function)`
  * `ctx.go_tests(package)`
  * `ctx.ts_components()`
  * `ctx.string_literals().matching(...)`
  * `ctx.jsx_attributes().matching(...)`
  * `ctx.report(diagnostic)`
  * Convenience helpers: `ctx.error(span, message)`, `ctx.warn(span, message)`.

`polint-go`

* Go language adapter.
* First implementation should use `tree-sitter-go`.
* Extract:

  * files
  * package names
  * imports
  * functions
  * methods
  * test functions
  * subtests
  * table tests where practical
  * basic branch obligations from `if`, `switch`, `for`, `range`, `select`, and `case/default`
  * basic CFG for functions
  * cyclomatic complexity
* Do not attempt full Go type checking in the first pass.
* Design an optional future Go semantic sidecar using `go/packages` or `go/analysis`.
* Leave a clean trait boundary for exact type information later.

`polint-ts`

* TypeScript/JavaScript adapter using Oxc.
* Support `.ts`, `.tsx`, `.js`, `.jsx`.
* Extract:

  * imports/exports
  * functions
  * classes
  * React-ish component functions where reasonable
  * JSX attributes
  * string literals
  * basic cyclomatic complexity
  * basic import graph
* Implement a practical no-raw-colors example rule using this adapter.

`polint-graph`

* Graph helpers around `petgraph`.
* Import graph.
* Basic call graph where possible.
* CFG representation.
* DOT export for debugging.

`polint-rules`

* Built-in example rules.
* These are not the main product.
* Keep built-in rules focused on proving and dogfooding the custom-rule SDK.
* Do not try to become a comprehensive ruleset.
* Built-ins should be examples of what users can write themselves.

Primary architecture:

```text id="04jcee"
CLI
  -> load config
  -> discover repo-local rules
  -> discover files
  -> hash files/config/rules
  -> parse changed files
  -> build requested analysis facts
  -> run custom rules and example built-in rules
  -> collect diagnostics
  -> render output
```

The analysis model should be fact-oriented.

Core structs should look roughly like this:

```rust id="g4o1wx"
pub struct FileId(pub u32);
pub struct FunctionId(pub u64);
pub struct BranchId(pub u64);

pub struct Span {
    pub file: FileId,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

pub struct FunctionFact {
    pub id: FunctionId,
    pub file: FileId,
    pub name: String,
    pub span: Span,
    pub language: Language,
    pub is_test: bool,
    pub is_exported: bool,
}

pub struct BranchObligation {
    pub id: BranchId,
    pub function: FunctionId,
    pub decision_span: Span,
    pub condition_text: String,
    pub edge_label: String,
    pub stable_fingerprint: String,
}
```

Diagnostics should look roughly like this:

```rust id="rwjytn"
pub struct Diagnostic {
    pub rule_id: String,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub labels: Vec<Label>,
    pub help: Option<String>,
    pub evidence: Vec<Evidence>,
    pub fix: Option<Fix>,
    pub stable_fingerprint: String,
}
```

Important UX requirements:

The tool should feel like a framework for writing custom linters inside a repo.

A user should be able to run:

```bash id="aznu3z"
polint init
polint new-rule go require-payment-error-tests
polint check
```

This should create something like:

```text id="llaj2e"
.polint/
  rules/
    require-payment-error-tests/
      Cargo.toml
      src/lib.rs
.polint.toml
```

Example generated rule skeleton:

```rust id="3y00tr"
use polint_sdk::prelude::*;

pub struct RequirePaymentErrorTests;

impl Rule for RequirePaymentErrorTests {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "custom/require-payment-error-tests",
            description: "Require payment error branches to have test evidence.",
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new()
            .go_tests()
            .branch_obligations()
    }

    fn run(&self, ctx: &mut RuleCtx) -> Result<()> {
        for function in ctx.functions().matching("internal/payments/**/*.go") {
            let obligations = ctx.branch_obligations(function)?;

            for obligation in obligations {
                if obligation.condition_text.contains("err != nil")
                    && !ctx.go_tests().has_evidence_for_branch(obligation.id)?
                {
                    ctx.report(
                        Diagnostic::error(
                            self.meta().id,
                            obligation.decision_span,
                            format!(
                                "No test evidence found for error branch `{}`",
                                obligation.condition_text
                            ),
                        )
                        .with_help(
                            "Add a test case that exercises this error branch and asserts the returned error."
                        ),
                    );
                }
            }
        }

        Ok(())
    }
}
```

A user should be able to add config like:

```toml id="fe5rjk"
# .polint.toml

[workspace]
include = ["internal/**", "apps/web/**"]
exclude = ["**/vendor/**", "**/node_modules/**", "**/*.pb.go"]

[rules]
paths = [".polint/rules"]

[profiles.fast]
rules = [
  "custom/*",
  "examples/ts-no-raw-colors"
]

[profiles.full]
rules = [
  "custom/*",
  "examples/*"
]

[[rules]]
id = "custom/require-payment-error-tests"
severity = "error"

[[rules]]
id = "examples/ts-no-raw-colors"
severity = "error"
files = ["apps/web/**/*.{ts,tsx}"]
allow_files = [
  "apps/web/src/theme/**",
  "apps/web/src/design-tokens/**"
]
```

The tool should still ship with example rules and useful helpers, but the docs and UX should make clear:

* This is not a replacement for ESLint/golangci-lint/etc.
* Keep using your normal language linters and formatters.
* Use this tool for project-specific policy and deeper custom analysis.
* The most important feature is making custom static-analysis rules easy to write, test, run, and share inside a repo.

Example user workflow:

```bash id="3wbf4m"
# Initialize framework config
polint init

# Generate a repo-local rule
polint new-rule go branch-error-paths

# Edit .polint/rules/branch-error-paths/src/lib.rs

# Test the rule against fixtures
polint test-rules

# Run locally
polint check --profile fast

# Run deeper checks in CI
polint check --profile full --format sarif > polint.sarif
```

Example human diagnostic:

```text id="c8jknd"
internal/payments/authorize.go:42:8 error custom/require-payment-error-tests

No test evidence found for error branch:
  function: AuthorizePayment
  condition: err != nil

Why this matters:
  This branch returns an error from a critical payment path, but no nearby test appears to exercise it.

Suggested action:
  Add a table test case that forces this error and asserts the returned error.
```

Example JSON diagnostic:

```json id="f24ygm"
{
  "rule_id": "custom/require-payment-error-tests",
  "severity": "error",
  "file": "internal/payments/authorize.go",
  "range": {
    "start_line": 42,
    "start_col": 8,
    "end_line": 44,
    "end_col": 5
  },
  "message": "No test evidence found for error branch `err != nil`.",
  "help": "Add a table test case that forces this error and asserts the returned error.",
  "stable_fingerprint": "custom-error-branch:authorizepayment:err-not-nil"
}
```

Example built-in/helper rules to implement first:

These are example rules and SDK dogfooding, not the main product.

1. `examples/go-cyclomatic-complexity`

* Calculate cyclomatic complexity for Go functions.
* Count decision points:

  * `if`
  * `for`
  * `range`
  * `case`
  * `default`
  * logical `&&` / `||` where practical
* Configurable by file glob and max value.
* Emit a breakdown of decision points if possible.

2. `examples/ts-cyclomatic-complexity`

* Same concept for TS/JS functions using Oxc AST.
* Support file globs and max value.

3. `examples/go-import-boundaries`

* Build import graph.
* Enforce that packages/files in one region do not import forbidden regions.
* Useful for architecture enforcement.

4. `examples/ts-no-raw-colors`

* Detect raw color literals in TS/TSX:

  * `#fff`
  * `#ffffff`
  * `#ffffffff`
  * `rgb(...)`
  * `rgba(...)`
  * `hsl(...)`
  * `hsla(...)`
* Allow configured files such as theme/token files.
* Emit a diagnostic telling the user to use design tokens or CSS variables.

5. `examples/go-branch-obligations`

* Extract branch obligations from Go functions.
* Initial version can be static only.
* For each `if`, create true/false obligations.
* For each `switch`, create obligations for each `case` and `default`.
* For `if err != nil`, mark as an error-path obligation.
* Match obligations against visible tests heuristically:

  * test functions in same package
  * table test names and fields
  * subtest names
  * asserted errors
* Be honest in diagnostic language if the rule is heuristic:

  * “No nearby test evidence found for this branch”
  * not “definitely untested”
* Design the data model so a future dynamic coverage collector can provide exact hits.

6. `examples/go-test-suite-size`

* Compute a weighted maintainability score for Go test suites:

  * test function count
  * subtest count
  * table row count
  * fixture file count
  * golden file count
  * external service/fake usage if detectable
  * global setup size
* Configurable max weight.
* Emit the largest contributors.

7. `examples/go-assertion-after-action`

* Detect tests that call a function but have no obvious assertion/error check.
* Heuristic is okay in v1.
* Recognize common Go patterns:

  * `if got != want`
  * `require.*`
  * `assert.*`
  * `t.Fatal`
  * `t.Errorf`
  * explicit error checks
* Emit useful diagnostics, but default severity should be `warn`.

8. `examples/config-query-no-literal`

* A generic configurable literal-deny rule that works across supported languages where possible.
* Lets users deny string literals or regex matches by file glob.

Custom rules:

Support repo-local Rust rules as a primary product goal.

Desired user layout:

```text id="yiellw"
.polint/
  rules/
    no_large_test_suites/
      Cargo.toml
      src/lib.rs
    branch_obligations/
      Cargo.toml
      src/lib.rs
```

Example custom rule:

```rust id="bwczng"
use polint_sdk::prelude::*;

pub struct NoLargeTestSuites;

impl Rule for NoLargeTestSuites {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "custom/no-large-test-suites",
            description: "Warn when a Go package has an overly large test suite.",
            severity: Severity::Warn,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new()
            .go_tests()
            .test_suite_metrics()
    }

    fn run(&self, ctx: &mut RuleCtx) -> Result<()> {
        for package in ctx.go_packages().matching("internal/**") {
            let suite = ctx.test_suite(package)?;

            let weight =
                suite.test_functions.len() as f64 * 1.0
                + suite.subtests.len() as f64 * 0.4
                + suite.table_rows.len() as f64 * 0.15
                + suite.fixture_files.len() as f64 * 4.0
                + suite.external_services.len() as f64 * 8.0;

            if weight > 120.0 {
                ctx.report(Diagnostic::warning(
                    "custom/no-large-test-suites",
                    suite.span,
                    format!("Test suite is too large: weight {:.1}, max 120.0", weight),
                ).with_help(
                    "Split this package into more focused behavior-specific test suites."
                ));
            }
        }

        Ok(())
    }
}
```

The first implementation may not fully auto-compile repo-local Rust rules. But the architecture must be shaped around this. At minimum:

* Implement the `Rule` trait and SDK.
* Implement example built-in rules using the same SDK.
* Add a documented path for registering native rules.
* Add `polint new-rule`.

Repo-local rules ship as normal Rust packages under `.polint/rules` (see workspace layout). Do not over-engineer the first pass. Make the rule-authoring experience excellent first.

Implementation order:

Phase 0: Repo setup

1. Create workspace and crates.
2. Set Rust edition.
3. Add dependencies.
4. Add CI-friendly commands:

   * `cargo fmt`
   * `cargo clippy --workspace --all-targets -- -D warnings`
   * `cargo test --workspace`
5. Add README with project goal, quickstart, and examples.

Phase 1: CLI and config

1. Implement `polint init`.
2. Implement `polint new-rule`.
3. Implement `.polint.toml` loading.
4. Implement `polint check`.
5. Implement include/exclude file discovery.
6. Implement human and JSON output.
7. Add integration tests for CLI.

Phase 2: Core analysis database

1. Add `FileId`, `Span`, `SourceFile`, `AnalysisDb`.
2. Add deterministic file ordering.
3. Add file content hashing.
4. Add language detection.
5. Add diagnostic model.
6. Add rule registry and rule runner.
7. Add deterministic diagnostic sorting.

Phase 3: Go adapter

1. Parse Go files with tree-sitter-go.
2. Extract package names, imports, functions, methods, and tests.
3. Extract `if` and `switch` branch obligations.
4. Implement Go cyclomatic complexity.
5. Implement basic Go import graph.
6. Add tests with fixtures.

Phase 4: TypeScript adapter

1. Parse TS/JS/TSX/JSX with Oxc.
2. Extract imports, functions, classes, JSX string attributes, and string literals.
3. Implement TS cyclomatic complexity.
4. Implement raw color detection.
5. Add tests with fixtures.

Phase 5: Example built-in rules

1. Implement `examples/go-cyclomatic-complexity`.
2. Implement `examples/ts-cyclomatic-complexity`.
3. Implement `examples/go-import-boundaries`.
4. Implement `examples/ts-no-raw-colors`.
5. Implement `examples/go-branch-obligations` static heuristic.
6. Implement `examples/go-test-suite-size`.
7. Implement `examples/go-assertion-after-action`.
8. Implement `examples/config-query-no-literal`.
9. Add snapshot tests for diagnostics.

Phase 6: Rule SDK

1. Refactor example built-in rules to use `polint-sdk`.
2. Make `RuleCtx` ergonomic.
3. Add examples showing custom native rules.
4. Add docs for rule authors.
5. Add a `polint test-rules` command that runs fixture-based rule tests.

Phase 7: Caching and performance

1. Add parse cache.
2. Add facts cache where practical.
3. Parallelize parsing and rule execution.
4. Add `polint profile-rules`.
5. Add performance tests on synthetic repos.
6. Ensure output remains deterministic under parallel execution.

Phase 8: SARIF and CI

1. Add SARIF-like output.
2. Add GitHub Actions example.
3. Add nonzero exit code on error-level diagnostics.
4. Add `--fail-on warn|error|none`.

Testing requirements:

The project must be very well tested.

Add unit tests for:

* config parsing
* custom rule discovery
* `polint new-rule` file generation
* glob matching
* file discovery
* span/line-column conversion
* diagnostic sorting
* Go function extraction
* Go import extraction
* Go test extraction
* Go branch obligation extraction
* Go cyclomatic complexity
* TS import extraction
* TS string literal extraction
* TS raw color detection
* TS cyclomatic complexity
* import-boundary matching
* test-suite weight calculation

Add integration tests for:

* `polint init`
* `polint new-rule`
* `polint check` on a clean fixture
* `polint check` on a failing Go fixture
* `polint check` on a failing TS fixture
* `polint check --format json`
* `polint check --profile fast`
* `polint check --profile full`
* exit codes
* cache on/off behavior

Add snapshot tests using `insta` for diagnostics:

* human output
* JSON output
* SARIF-like output

Add property tests where useful:

* Span conversion should roundtrip byte offsets to line/column for valid UTF-8.
* Diagnostic sorting should be stable and deterministic.
* File discovery should not include excluded paths.

Performance requirements:

* The tool must be fast on large repos.
* Use parallel parsing.
* Avoid cloning large source strings.
* Store source text once per file.
* Use stable IDs instead of copying AST fragments.
* Avoid passing full ASTs to rules unless necessary.
* Rule APIs should expose high-level facts.
* Cache by file hash, config hash, and rule hash.
* Make expensive analyzers capability-gated.
* A rule must declare capabilities so the engine only computes needed facts.
* Add `--profile-rules` to show per-rule time.

Capability model:

Rules declare capabilities:

```rust id="ez9q7x"
Capabilities::new()
    .syntax()
    .imports()
    .cfg()
    .call_graph()
    .go_tests()
    .branch_obligations()
    .coverage_facts()
```

The engine should compute only the requested facts.

Profiles:

Support config profiles:

```toml id="kbjmtz"
[profiles.fast]
rules = ["custom/*", "examples/ts-no-raw-colors"]

[profiles.full]
rules = ["custom/*", "examples/*"]
```

Default:

* `polint check` uses `fast`.
* CI docs should recommend `polint check --profile full`.

Important behavior:

* If config is missing, `polint check` should still run a minimal default and suggest `polint init`.
* Diagnostics must be deterministic.
* Errors in one file should not prevent analysis of unrelated files unless fatal.
* Parser errors should be diagnostics, not panics.
* All panics in rule execution should be caught if possible and reported as internal rule errors.
* Use stable diagnostic fingerprints.
* Exit code:

  * `0` if no diagnostics at or above fail threshold.
  * `1` if diagnostics at or above fail threshold.
  * `2` for tool/config/internal fatal errors.

README requirements:

Write a README explaining:

1. What the tool is:

   * A high-performance Rust framework for writing custom static-analysis rules inside a repo.
   * Initially supports Go and TypeScript.
   * Rules are code.
   * The framework provides analysis helpers and diagnostics.
   * It is not intended to replace normal linters and formatters.

2. Why it exists:

   * AI-assisted coding often violates local project-specific conventions.
   * Prompting the AI repeatedly is unreliable.
   * Encoding expectations as repo-local executable checks is reliable.
   * Custom static analysis is powerful when it is easy to write, test, and run.

3. Quickstart:

   * install
   * init
   * new-rule
   * check
   * example config

4. Example custom rules:

   * no raw TS colors
   * Go domain complexity
   * Go import boundaries
   * Go branch obligations
   * Go test suite size
   * project-specific error-path testing

5. Custom rule authoring:

   * Show a small Rust custom rule using SDK.
   * Explain that example built-in rules use the same API.
   * Explain capabilities.
   * Explain rule testing.

6. CI:

   * GitHub Actions example.

7. Roadmap:

   * exact Go semantic sidecar
   * dynamic branch coverage instrumentation
   * more languages

Deliverables:

* A compiling Rust workspace.
* Working CLI.
* Working config.
* Working `polint new-rule`.
* Working Go parser/extractor.
* Working TS parser/extractor.
* Clean and ergonomic `polint-sdk`.
* At least these example rules working:

  * `examples/go-cyclomatic-complexity`
  * `examples/go-import-boundaries`
  * `examples/go-branch-obligations`
  * `examples/go-test-suite-size`
  * `examples/ts-no-raw-colors`
  * `examples/ts-cyclomatic-complexity`
* Human and JSON output.
* Meaningful tests and snapshots.
* README and examples.

Do not fake functionality. If a rule is heuristic, label it honestly in docs and diagnostics. Prefer a smaller working, well-tested implementation over a broad but shallow one.

The final result should feel like this:

```bash id="i5feuk"
polint init
polint new-rule go my-project-policy
polint check
```

And users should be able to say:

“I want this repo-specific engineering policy enforced,”

then express it as a small Rust rule inside their codebase, using powerful static-analysis helpers, without building an entire static-analysis tool themselves.
