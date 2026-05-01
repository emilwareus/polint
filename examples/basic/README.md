# Basic Example

Minimal flow for a repository that wants the default fast profile.

```bash
polint init
polint check
polint check --profile full --format sarif
```

From this repository during development, run the same commands through Cargo:

```bash
cargo run -p polint-cli -- init
cargo run -p polint-cli -- check
cargo run -p polint-cli -- check --profile full --format sarif
```

`polint init` creates `.polint.toml` and a `.polint/rules/` directory. The
default `polint check` command works even before you add repo-local rules, so it
is a quick way to verify file discovery and built-in example rules.
