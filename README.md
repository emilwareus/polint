# polint

**AI-agent-native, shadcn-style linting for rules you own.**

polint turns repo-specific engineering instructions into executable lint rules.
AI agents do not always follow prose in `CLAUDE.md`, `AGENTS.md`, prompts, or
review comments. polint lets you encode the parts that are actually analyzable.

Think shadcn, but for linting: you own the rule code in your repository; polint
brings the scaffolding and infrastructure to create, run, test, and ship it.

polint ships no built-in policy rules. It gives you the SDK, parsers, facts,
diagnostics, local rule runner, config, cache, and CI output so your repo can own
the rules.

## Quick Example

Say your frontend must use design tokens instead of raw colors. A polint rule in
your repo can catch the violation and tell the AI agent exactly how to fix it:

![polint diagnostic for a raw-color literal in Button.tsx](https://raw.githubusercontent.com/emilwareus/polint/main/docs/img/example-no-raw-colors.svg)

That is the point: the rule does not just fail the code. It injects the missing
project context back into the agent at the moment it needs to repair the change.

## Try It

Install polint:

```bash
cargo install polint --locked
```

Or from GitHub Releases:

```bash
curl -sSfL https://raw.githubusercontent.com/emilwareus/polint/main/scripts/install.sh | bash
```

Run a self-contained example:

```bash
git clone https://github.com/emilwareus/polint.git
cd polint/examples/config-denied-literal
polint check --color always --fail-on none
```

Expected output:

![polint check on examples/config-denied-literal showing a denied literal diagnostic](https://raw.githubusercontent.com/emilwareus/polint/main/docs/img/example-config-denied-literal.svg)

## Use It In Your Repo

```bash
polint init
polint add-skill
polint new-rule ts no-raw-colors
polint check
```

`polint init` creates `.polint.toml` and `.polint/rules/`.
`polint new-rule <go|ts|js|generic> <name>` adds a Rust rule module to your
local rule pack. `polint check` discovers and runs that rule pack.

Rule packs live in your repo:

```text
.polint.toml
.polint/
  rules/
    Cargo.toml
    src/
      main.rs
      no_raw_colors.rs
```

Profiles are explicit:

- `polint check` runs every discovered rule.
- `polint check --profile web` runs exactly `[profiles.web]`.
- Unknown profiles are errors.
- Profile names are arbitrary. There is no default profile.

## CI

```yaml
name: polint

on: [push, pull_request]

jobs:
  polint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install polint --locked
      - run: polint check --format sarif > polint.sarif
```

## More

- [Examples](examples/)
- [Analysis roadmap](docs/ANALYSIS-ROADMAP.md)
- [Release process](docs/RELEASING.md)

Rust **1.95** is pinned in [`rust-toolchain.toml`](rust-toolchain.toml).

## License

[MIT](LICENSE)
