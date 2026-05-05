use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Args, Clone)]
pub struct AddSkillArgs {
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

pub fn add_skill(root: PathBuf, args: &AddSkillArgs) -> Result<()> {
    let agents = selected_agents(args)?;
    for agent in agents {
        let skill_path = install_skill(&root, agent, args.force)?;
        println!(
            "Installed {} skill at {}",
            agent.label(),
            display_relative(&root, &skill_path)
        );
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

fn install_skill(root: &Path, agent: SkillAgent, force: bool) -> Result<PathBuf> {
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
    if skill_path.exists() && !force {
        anyhow::bail!(
            "skill already exists: {} (use --force to overwrite)",
            skill_path.display()
        );
    }

    fs::create_dir_all(&skill_dir)
        .with_context(|| format!("failed to create {}", skill_dir.display()))?;
    fs::write(&skill_path, skill_markdown(agent))
        .with_context(|| format!("failed to write {}", skill_path.display()))?;
    Ok(skill_path)
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

Use `polint check --format json` when you need machine-readable diagnostics and
`polint check --format sarif` for CI upload paths. Use `--fail-on warn`, `error`,
or `none` to control the exit status.

## Rule Layout

Single-rule repositories usually use one Cargo manifest per rule:

```text
.polint.toml
.polint/
      rules/
    no-raw-colors/
      Cargo.toml
      src/main.rs
```

For multiple local rules that should be compiled and run together, use one
rule-pack Cargo manifest under `.polint/rules/Cargo.toml` and register each rule
from that host. See `examples/multiple-rules` in the polint repository for the
shape.

## Writing A Rule

Start with `use polint_sdk::prelude::*;`, register the rule with
`polint_runner::run_cli`, give the rule a stable local ID, declare only the facts
it needs in `capabilities`, then report diagnostics from `run`.

```rust
use polint_sdk::prelude::*;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {{
    polint_runner::run_cli(vec![Arc::new(NoRawColors)])
}}

struct NoRawColors;

impl Rule for NoRawColors {{
    fn meta(&self) -> RuleMeta {{
        RuleMeta {{
            id: "local/no-raw-colors".to_string(),
            description: "Require design tokens instead of raw color literals.".to_string(),
            severity: Severity::Error,
        }}
    }}

    fn capabilities(&self) -> Capabilities {{
        Capabilities::new().string_literals().jsx_attributes()
    }}

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {{
        for literal in ctx.string_literals() {{
            if literal.value.starts_with('#') {{
                ctx.report(
                    Diagnostic::error(
                        self.meta().id,
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
}}
```

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
- Keep rules small and specific to the repository convention they enforce.
- State when a rule is heuristic, especially for test evidence or branch coverage.
- Prefer parser facts and SDK helpers over ad hoc text scanning.
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

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
