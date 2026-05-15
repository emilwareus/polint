# Consumer setup and troubleshooting

## Rust toolchain

polint and repo-local rule crates target the MSRV in the workspace `Cargo.toml`.
Rule packs use **Rust 2024**.

If `polint check` fails while compiling `.polint/rules` with an MSRV error, align
`rust-toolchain.toml` (or your CI image) with that MSRV, or pin the compiler for
the child `cargo` process (see below).

## Go symbol/reference setup

Rules that request Go `Symbols<'_>` or `References<'_>` use the embedded
`polint-go-symbols` sidecar. The current implementation requires:

- Go 1.24 or newer on `PATH` when using the default embedded source sidecar
- each analyzed Go file to belong to a Go module with a `go.mod`
- package loading to succeed for the configured package patterns and build tags

If those requirements are missing, polint emits `polint/capability` diagnostics
and blocks the Go symbol/reference rules instead of running them with placeholder
facts.

For simple repositories, no Go-specific config is needed. polint walks from each
discovered Go file to the nearest `go.mod` and loads those module roots. For
monorepos, keep the setup in the single `.polint.toml` file when you want an
explicit lifecycle:

```toml
[languages.go]
module_roots = ["services/payments", "libs/money"]
package_patterns = ["./..."]
build_tags = ["enterprise"]
include_tests = true
```

`package_patterns` are interpreted inside each configured module root. If the
repository has a root `go.work` that covers every selected module root, polint
uses it. Otherwise, when package loading needs workspace mode for module roots
below the repository root, polint creates a temporary internal `go.work`; it does
not write another setup file into the repository.

## Environment variables

| Variable | Effect |
|----------|--------|
| `POLINT_CARGO` | Executable used to spawn repo-local rule hosts (default: `cargo` or `CARGO`). |
| `POLINT_CACHE_DIR` | Optional cache root. Defaults to `.polint/cache` relative to the checked repository. |
| `POLINT_GO_SYMBOLS` | Optional path to a `polint-go-symbols` binary or sidecar source directory. A binary can avoid requiring Go on `PATH`; a source directory still needs Go. |
| `POLINT_RULES_TARGET_DIR` | Optional Cargo target directory for repo-local rule hosts. Defaults to `$POLINT_CACHE_DIR/rules-target`. |
| `POLINT_RULES_TOOLCHAIN` | When set to a non-empty value, forwarded as `RUSTUP_TOOLCHAIN` for the rules-host `cargo run` subprocess (parent `polint check` only). |
| `NO_COLOR` | Disables ANSI colors when `--color auto`. |

## Cache management

polint stores local cache data under `.polint/cache` unless `POLINT_CACHE_DIR`
is set:

| Path | Contents |
|------|----------|
| `.polint/cache/analysis` | Compact JSON parser/fact artifacts. |
| `.polint/cache/rules-target` | Cargo target directory used when `polint check` builds `.polint/rules`. |
| `.polint/cache/derived` | Reserved for future project-level derived facts. |

Analysis cache keys include source path and content, loaded config, rule/options
digest, requested capability plan, cache format, and polint version. Changing
those inputs produces fresh cache entries instead of reusing stale facts.
If an individual cache artifact cannot be decoded, polint treats it as a miss
and removes that artifact.

Use `polint cache status` to inspect size and file counts. The JSON form is
stable enough for scripts and follows
[`polint-cache-status-v1.json`](schemas/polint-cache-status-v1.json):

```bash
polint cache status --format json
```

Use explicit cleanup when the cache grows too large:

```bash
polint cache prune --max-size-mb 512
polint cache prune --max-age-days 14 --dry-run
polint cache clean --category analysis
polint cache clean --category rules-target
polint cache clean
```

`--no-cache` on `polint check`, `polint baseline`, and `polint ignores` disables
analysis/fact cache reads and writes for that run. It does not disable the
repo-local rule-host Cargo target cache; use `polint cache clean --category
rules-target` when you need a fresh rule-host build.

In GitHub Actions, prefer the official action, which installs polint and
restores/saves `.polint/cache` by default:

```yaml
- uses: emilwareus/polint@v1
  with:
    version: latest
    args: check --format github
```

If you wire the steps manually, cache `.polint/cache` when repo-local rules are
enabled:

```yaml
- uses: actions/cache@v4
  with:
    path: .polint/cache
    key: polint-${{ runner.os }}-${{ hashFiles('.polint.toml', '.polint/rules/Cargo.lock', '.polint/rules/**/*.rs') }}
    restore-keys: |
      polint-${{ runner.os }}-
```

The first run for a new cache key may still compile `.polint/rules` and
populate analysis artifacts. The cache primarily improves repeat CI runs.

## Rules host failures

When the parent CLI runs `cargo run --manifest-path …/.polint/rules/Cargo.toml`,
failures are reported on stderr with the prefix:

`polint: rules host:`

Follow-up hints may mention:

- **MSRV** — polint library requires the workspace MSRV; see stderr and `rustc -V`.
- **Network / registry** — dependency fetch failures (VPN, offline, crates.io).
- **Manifest** — invalid `Cargo.toml` or workspace layout under `.polint/rules`.
- **Missing rustc** — install Rust or set `POLINT_RULES_TOOLCHAIN`.

See also the [README](../README.md) **Versions** table.

## SARIF rule metadata

Optional map in `.polint.toml`:

```toml
[sarif.rule_help_uri]
"local/my-rule" = "https://example.com/docs/my-rule"
```

Values become SARIF `reportingDescriptor.helpUri` for matching `rule_id`s.

## Rule-specific settings

Each `[[rules.config]]` table supports common shortcuts (`severity`, `files`,
`allow_files`, `allow`, `max`, `deny`, `forbidden_imports`) plus arbitrary
rule-owned fields. Unknown fields are preserved in `ctx.options().settings`.

```toml
[[rules.config]]
id = "local/no-placeholder-literals"
files = ["src/**/*.ts"]
literal = "TODO"
message = "Replace placeholder literals before merging."
```

```rust
let literal = ctx
    .options()
    .settings
    .get("literal")
    .and_then(|value| value.as_str())
    .unwrap_or("TODO");
```

## Comment ignores

polint supports source comments for suppressing policy diagnostics:

```ts
// polint-ignore-next-line local/no-placeholder-literals -- generated fixture
const status = "TODO";
```

Selectors are required and use the same exact / `prefix/*` / `*` matching as
profiles. Repositories can require reasons:

```toml
[ignores]
require_reason = true
```

Use `polint ignores --stat` or `polint ignores --format json` to inspect active,
unused, malformed, and missing-reason ignores. See
[IGNORE-COMMENTS.md](IGNORE-COMMENTS.md).

## Monorepo path pairing

Optional section pairs left/right path shapes that share a context segment (same
string between configured prefix/suffix markers). See `[path_contexts]` in
`.polint.toml` and `RuleCtx::path_context_related` in the SDK after analysis.
