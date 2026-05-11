use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Args, Clone)]
pub(crate) struct AddSkillArgs {
    /// AI agent to install the skill for. Repeat to install for multiple agents.
    #[arg(long = "agent", value_enum)]
    agents: Vec<SkillAgent>,

    /// Install the skill for every supported agent.
    #[arg(long)]
    all: bool,

    /// Overwrite an existing polint skill.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum SkillAgent {
    Claude,
    Codex,
}

pub(crate) fn add_skill(root: PathBuf, args: &AddSkillArgs) -> Result<()> {
    let agents = selected_agents(args)?;
    for agent in agents {
        match install_skill(&root, agent, args.force)? {
            SkillInstall::Installed(skill_path) => {
                println!(
                    "Installed {} skill at {}",
                    agent.label(),
                    display_relative(&root, &skill_path)
                );
            }
            SkillInstall::Skipped(skill_path) => {
                println!(
                    "Kept existing {} skill at {}",
                    agent.label(),
                    display_relative(&root, &skill_path)
                );
            }
        }
    }
    Ok(())
}

fn selected_agents(args: &AddSkillArgs) -> Result<Vec<SkillAgent>> {
    if args.all {
        return Ok(all_agents());
    }
    if !args.agents.is_empty() {
        return Ok(dedupe_agents(&args.agents));
    }
    select_agents_interactively()
}

fn select_agents_interactively() -> Result<Vec<SkillAgent>> {
    println!("Install the polint skill for which AI agent?");
    println!("  1) Claude Code  (.claude/skills/polint/SKILL.md)");
    println!("  2) Codex        (.agents/skills/polint/SKILL.md)");
    println!("  3) Both");
    print!("Select agents [3]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    parse_agent_selection(input.trim())
}

fn parse_agent_selection(input: &str) -> Result<Vec<SkillAgent>> {
    if input.is_empty() {
        return Ok(all_agents());
    }

    let normalized = input.to_ascii_lowercase();
    if matches!(normalized.as_str(), "3" | "all" | "both" | "a") {
        return Ok(all_agents());
    }

    let mut selected = Vec::new();
    for token in normalized
        .split([',', ' ', '\t'])
        .filter(|token| !token.is_empty())
    {
        match token {
            "1" | "claude" | "claude-code" => selected.push(SkillAgent::Claude),
            "2" | "codex" => selected.push(SkillAgent::Codex),
            "3" | "all" | "both" => return Ok(all_agents()),
            _ => {
                anyhow::bail!(
                    "unknown agent selection `{token}`; choose 1, 2, 3, claude, codex, or both"
                );
            }
        }
    }

    if selected.is_empty() {
        anyhow::bail!("no agents selected");
    }
    Ok(dedupe_agents(&selected))
}

enum SkillInstall {
    Installed(PathBuf),
    Skipped(PathBuf),
}

fn install_skill(root: &Path, agent: SkillAgent, force: bool) -> Result<SkillInstall> {
    let target_dir = target_skill_dir(root, agent);
    let skill_dir = target_dir.join("polint");
    let skill_path = skill_dir.join("SKILL.md");

    ensure_safe_repo_path(root, &skill_dir)?;
    ensure_safe_repo_path(root, &skill_path)?;
    if let Ok(metadata) = fs::symlink_metadata(&skill_path)
        && metadata.file_type().is_symlink()
    {
        anyhow::bail!("refusing to overwrite symlink: {}", skill_path.display());
    }
    if skill_path.exists() && !force && !confirm_overwrite(&skill_path)? {
        return Ok(SkillInstall::Skipped(skill_path));
    }

    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("failed to create {}", skill_dir.display()))?;
    fs::write(&skill_path, skill_markdown(agent))
        .with_context(|| format!("failed to write {}", skill_path.display()))?;
    Ok(SkillInstall::Installed(skill_path))
}

fn confirm_overwrite(skill_path: &Path) -> Result<bool> {
    println!("polint skill already exists at {}", skill_path.display());
    print!("Overwrite existing polint skill? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn target_skill_dir(root: &Path, agent: SkillAgent) -> PathBuf {
    for relative in agent.candidate_skill_dirs() {
        let candidate = root.join(relative);
        if candidate.is_dir() {
            return candidate;
        }
    }
    root.join(agent.default_skill_dir())
}

fn ensure_safe_repo_path(root: &Path, target: &Path) -> Result<()> {
    let relative = target
        .strip_prefix(root)
        .with_context(|| format!("target path must stay inside {}", root.display()))?;
    let mut current = root.to_path_buf();

    for component in relative.components() {
        match component {
            Component::Normal(part) => current.push(part),
            Component::CurDir => {}
            _ => anyhow::bail!("unsafe skill path component in {}", target.display()),
        }

        if let Ok(metadata) = fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            anyhow::bail!("refusing to write through symlink: {}", current.display());
        }
    }

    Ok(())
}

fn skill_markdown(agent: SkillAgent) -> String {
    let allowed_tools = match agent {
        SkillAgent::Claude => {
            "\nallowed-tools: Bash(polint:*) Bash(cargo:*) Read Write Edit MultiEdit Glob Grep LS"
        }
        SkillAgent::Codex => "",
    };
    format!(
        r#"---
name: polint
description: Use polint to write and run repo-local static-analysis policy rules.{allowed_tools}
---

# polint Repo-Local Policy Rules

Use this skill when the user wants project-specific linting rules, policy checks,
or static analysis that generic tools cannot know. polint ships no built-in
policy rules; every policy belongs to the repository that needs it.

## Fast Workflow

```bash
polint init
polint new-rule go require-error-branch-tests
polint new-rule ts no-raw-colors
polint check --fail-on none
```

Use `polint check --format json` when you need machine-readable diagnostics. JSON
is a versioned report object with a `diagnostics` array (not a bare array at the
root); the schema lives in `docs/schemas/polint-report-v1.json` in the polint repo.
Human output uses ANSI colors on a TTY unless `NO_COLOR` is set; use `--color never`
for plain text. Use `polint check --format sarif` for CI upload paths. Use
	`--fail-on warn`, `error`, or `none` to control the exit status. Use `polint check
	--shortstat` or `polint check --stat` for human scan summaries; these flags do
	not add prose to JSON or SARIF output.

Use a compact YAML baseline at `.polint/baseline.yaml` when existing findings
should not block new work:

```bash
polint baseline create
polint check --baseline --new-only
polint baseline update
```

The baseline file has one string per entry:

```yaml
version: 1

baseline:
  - "local/backend-context-propagation e337fbb73d44b2b7 backend/app/handler.go"
ignore:
  - "local/no-raw-colors 1b7c9a00e493aa21 frontend/Button.tsx"
```

`baseline` is existing debt; it stays visible but does not fail. `ignore` is a
central accepted suppression; it is hidden from output and failure. Matching uses
`rule_id + fingerprint`, with the file path kept for reviewability.

Use `polint ignores` when you need to find suppressions that should be fixed:

```bash
polint ignores --shortstat
polint ignores --stat --filter local/no-raw-colors,local/*
polint ignores --format json --filter local/no-raw-colors
```

Ignore comments look like
`// polint-ignore-next-line local/no-raw-colors -- legacy fixture`. Selectors are
required. Ignores suppress policy diagnostics only; parser, internal,
capability, and `polint/*` diagnostics stay visible. Repositories can require
reasons with `[ignores] require_reason = true` in `.polint.toml`.

## Rule Layout

Repo-local rules live in **one** Rust package under `.polint/rules/`:

```text
.polint.toml
.polint/rules/Cargo.toml
.polint/rules/src/main.rs          # calls polint::runner::run_cli(vec![...])
.polint/rules/src/my_rule.rs       # one #[polint::rule] function per rule
```

`polint new-rule <lang> <name>` adds `src/<name_with_underscores>.rs` and wires it
into `src/main.rs`. See `examples/multiple-rules` in the polint repo for several
rules in one pack.

## Writing A Rule

Start with `use polint::sdk::prelude::*;`, register the rule with
`polint::runner::run_cli`, give the rule a stable local ID in `#[polint::rule]`,
and request facts as typed fact-view parameters. polint derives the rule's
capabilities from those parameter types.
Use `ctx.options().settings` for rule-specific TOML fields that are not covered
by the common shortcuts (`max`, `deny`, `forbidden_imports`, etc.).

`src/main.rs`:

```rust
use std::process::ExitCode;

mod no_raw_colors;

fn main() -> ExitCode {{
    polint::runner::run_cli(vec![no_raw_colors::no_raw_colors()])
}}
```

`src/no_raw_colors.rs`:

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-colors",
    description = "Require design tokens instead of raw color literals.",
    severity = "error"
)]
pub(crate) fn no_raw_colors(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {{
    for literal in literals.iter() {{
        if literal.value.starts_with('#') {{
            ctx.report(
                Diagnostic::error(
                    ctx.rule_id(),
                    ctx.file_path(literal.file),
                    literal.span.diagnostic_range(),
                    "Use a design token instead of a raw color literal.",
                )
                .with_evidence("literal", literal.value.clone()),
            );
        }}
    }}
    Ok(())
}}
```

## Reusable Metric Signals

For code-quality policies, prefer reusable signal views over rules calling other
rules. `FileMetrics<'_>` exposes file line/byte/function counts,
`FunctionMetrics<'_>` exposes per-function size, and `ComplexityMetrics<'_>`
exposes per-function syntax-level cyclomatic complexity. A composite rule can
request several of these typed views in one `#[polint::rule]` signature.

## Module Relationship Facts

For architecture policies, request `ResolvedImports<'_>` to inspect resolution
status and unresolved reasons, and request `ModuleGraphFacts<'_>` to inspect
file, package, module, and dependency edges. Both views are exported by
`polint::sdk::prelude::*`; keep rules on the typed fact-view path and treat
`SetupMissing`, `Dynamic`, and `Unsupported` statuses as meaningful data.

## Config Pattern

Profiles are explicit named subsets. `polint check` with no `--profile` runs
every discovered rule. Add a named profile only when the repository explicitly
needs a subset, and treat unknown profile names as errors:

```toml
[workspace]
include = ["src/**"]
exclude = ["**/node_modules/**", "**/vendor/**"]

[rules]
paths = [".polint/rules"]

[[rules.config]]
id = "local/no-raw-colors"
severity = "error"
files = ["src/**/*.{{ts,tsx}}"]
allow_files = ["src/theme/**"]
```

## Agent Rules

- Do not add project policies to the polint CLI as built-ins.
- Document only stable, supported CLI workflows; keep debug helpers, exploratory analysis surfaces, and future/TBD behavior out of generated skills until they are intentionally promoted.
- Keep rules small and specific to the repository convention they enforce.
- State when a rule is heuristic, especially for test evidence or branch coverage.
- Prefer parser facts and SDK helpers over ad hoc text scanning.
- Request typed fact views in the `#[polint::rule]` signature; examples are consumers of the SDK, not special internal entry points.
- Compose `FileMetrics<'_>`, `FunctionMetrics<'_>`, and `ComplexityMetrics<'_>` for higher-level quality rules instead of making rules depend on other rules.
- For architecture rules, compose `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` instead of parsing import strings yourself.
- Do not implement `Rule` manually or write handwritten capability declarations.
- For custom config, prefer explicit fields in `[[rules.config]]` and read them through `ctx.options().settings`.
- Add the smallest real fixture that demonstrates the policy violation.
- Run the rule through the CLI before claiming it works.
"#
    )
}

impl SkillAgent {
    fn label(self) -> &'static str {
        match self {
            SkillAgent::Claude => "Claude Code",
            SkillAgent::Codex => "Codex",
        }
    }

    fn default_skill_dir(self) -> &'static str {
        match self {
            SkillAgent::Claude => ".claude/skills",
            SkillAgent::Codex => ".agents/skills",
        }
    }

    fn candidate_skill_dirs(self) -> &'static [&'static str] {
        match self {
            SkillAgent::Claude => &[".claude/skills"],
            SkillAgent::Codex => &[".codex/skills", ".agents/skills"],
        }
    }
}

fn all_agents() -> Vec<SkillAgent> {
    vec![SkillAgent::Claude, SkillAgent::Codex]
}

fn dedupe_agents(agents: &[SkillAgent]) -> Vec<SkillAgent> {
    agents
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Repo-relative path for human-facing stdout (always `/` so copy-paste and tests match every OS).
fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
