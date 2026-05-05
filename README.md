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

```ansi
[1;31merror[0m[1m[[1;35mlocal/no-raw-colors[0m]: [1mRaw color literal `#1d4ed8` should use a design token.
  [0m[1;36m-->[0m [1mButton.tsx[0m:12:23-12:32
  [2mevidence[0m token_source: apps/web/src/theme/tokens.css
  [1;36mhelp:[0m Use `var(--color-action-primary)`. Do not define new colors in feature code.
```

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

```ansi
[1;31merror[0m[1m[[1;35mlocal/no-denied-literals[0m]: [1mConfigured denied literal `legacy-testid` found.
  [0m[1;36m-->[0m [1mquery.ts[0m:4:25-4:40
  [2m   |[0m
 [2m  4[0m [2m|[0m export const selector = "legacy-testid";
 [2m   [0m [2m|[0m [1;31m                        ^^^^^^^^^^^^^^^[0m
  [2mevidence[0m literal: legacy-testid
  [2mevidence[0m matched: legacy-testid
  [1;36mhelp:[0m Replace the literal with an allowed constant or local abstraction.
  [2mfingerprint: e337fbb73d44b2b7[0m
```

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
