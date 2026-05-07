# Consumer setup and troubleshooting

## Rust toolchain

polint and repo-local rule crates target the MSRV in the workspace `Cargo.toml`.
Rule packs use **Rust 2024**.

If `polint check` fails while compiling `.polint/rules` with an MSRV error, align
`rust-toolchain.toml` (or your CI image) with that MSRV, or pin the compiler for
the child `cargo` process (see below).

## Environment variables

| Variable | Effect |
|----------|--------|
| `POLINT_CARGO` | Executable used to spawn repo-local rule hosts (default: `cargo` or `CARGO`). |
| `POLINT_RULES_TOOLCHAIN` | When set to a non-empty value, forwarded as `RUSTUP_TOOLCHAIN` for the rules-host `cargo run` subprocess (parent `polint check` only). |
| `NO_COLOR` | Disables ANSI colors when `--color auto`. |

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

## Monorepo path pairing

Optional section pairs left/right path shapes that share a context segment (same
string between configured prefix/suffix markers). See `[path_contexts]` in
`.polint.toml` and `RuleCtx::path_context_related` in the SDK after analysis.
