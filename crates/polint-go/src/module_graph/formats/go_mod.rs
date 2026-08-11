use polint_analysis::module_graph::topology::{RequirementKind, TopologyPrecision, TopologyStatus};

pub(crate) const GO_MOD_SOURCE_LABEL: &str = "go.mod";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoModManifest {
    pub(crate) relative_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) module_path: Option<String>,
    pub(crate) go_version: Option<String>,
    pub(crate) requirements: Vec<GoRequirementDirective>,
    pub(crate) replacements: Vec<GoReplacementDirective>,
    pub(crate) excludes: Vec<GoRequirementDirective>,
    pub(crate) unsupported: Vec<GoUnsupportedDirective>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoRequirementDirective {
    pub(crate) target_name: String,
    pub(crate) version_requirement: Option<String>,
    pub(crate) kind: RequirementKind,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoReplacementDirective {
    pub(crate) target_name: String,
    pub(crate) version_requirement: Option<String>,
    pub(crate) replacement_target: String,
    pub(crate) replacement_version: Option<String>,
    pub(crate) kind: RequirementKind,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoUnsupportedDirective {
    pub(crate) directive: String,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

pub(crate) fn parse_go_mod(relative_path: &str, contents: &str) -> GoModManifest {
    let mut manifest = GoModManifest {
        relative_path: relative_path.to_string(),
        source_label: GO_MOD_SOURCE_LABEL,
        module_path: None,
        go_version: None,
        requirements: Vec::new(),
        replacements: Vec::new(),
        excludes: Vec::new(),
        unsupported: Vec::new(),
    };

    parse_directive_lines(contents, |directive, entry| {
        apply_go_mod_directive(&mut manifest, relative_path, directive, entry);
    });

    manifest
}

fn apply_go_mod_directive(
    manifest: &mut GoModManifest,
    relative_path: &str,
    directive: &str,
    entry: &str,
) {
    match directive {
        "module" => {
            if let Some(module_path) = single_token(entry) {
                manifest.module_path = Some(module_path);
            } else {
                manifest.unsupported.push(unsupported_directive(
                    relative_path,
                    GO_MOD_SOURCE_LABEL,
                    directive,
                ));
            }
        }
        "go" => {
            if let Some(go_version) = single_token(entry) {
                manifest.go_version = Some(go_version);
            } else {
                manifest.unsupported.push(unsupported_directive(
                    relative_path,
                    GO_MOD_SOURCE_LABEL,
                    directive,
                ));
            }
        }
        "require" => match parse_requirement_directive(
            relative_path,
            GO_MOD_SOURCE_LABEL,
            RequirementKind::Direct,
            entry,
        ) {
            Some(requirement) => manifest.requirements.push(requirement),
            None => manifest.unsupported.push(unsupported_directive(
                relative_path,
                GO_MOD_SOURCE_LABEL,
                directive,
            )),
        },
        "replace" => match parse_replacement_directive(relative_path, GO_MOD_SOURCE_LABEL, entry) {
            Some(replacement) => manifest.replacements.push(replacement),
            None => manifest.unsupported.push(unsupported_directive(
                relative_path,
                GO_MOD_SOURCE_LABEL,
                directive,
            )),
        },
        "exclude" => match parse_requirement_directive(
            relative_path,
            GO_MOD_SOURCE_LABEL,
            RequirementKind::Exclude,
            entry,
        ) {
            Some(exclude) => manifest.excludes.push(exclude),
            None => manifest.unsupported.push(unsupported_directive(
                relative_path,
                GO_MOD_SOURCE_LABEL,
                directive,
            )),
        },
        _ => {}
    }
}

pub(super) fn parse_directive_lines(contents: &str, mut handle: impl FnMut(&str, &str)) {
    let mut block_directive: Option<String> = None;
    for raw_line in contents.lines() {
        let line = strip_go_line_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(directive) = block_directive.as_deref() {
            let (entry, done) = split_block_entry(line);
            if !entry.is_empty() {
                handle(directive, entry);
            }
            if done {
                block_directive = None;
            }
            continue;
        }

        let Some((directive, rest)) = split_directive(line) else {
            continue;
        };
        let rest = rest.trim_start();
        if let Some(block_rest) = rest.strip_prefix('(') {
            let (entry, done) = split_block_entry(block_rest);
            if !entry.is_empty() {
                handle(directive, entry);
            }
            if !done {
                block_directive = Some(directive.to_string());
            }
        } else {
            handle(directive, rest);
        }
    }
}

pub(super) fn parse_requirement_directive(
    relative_path: &str,
    source_label: &'static str,
    kind: RequirementKind,
    entry: &str,
) -> Option<GoRequirementDirective> {
    let tokens = go_tokens(entry);
    if tokens.len() != 2 {
        return None;
    }
    Some(GoRequirementDirective {
        target_name: tokens[0].clone(),
        version_requirement: Some(tokens[1].clone()),
        kind,
        source_path: relative_path.to_string(),
        source_label,
        precision: TopologyPrecision::ExactStatic,
        status: TopologyStatus::Present,
    })
}

pub(super) fn parse_replacement_directive(
    relative_path: &str,
    source_label: &'static str,
    entry: &str,
) -> Option<GoReplacementDirective> {
    let tokens = go_tokens(entry);
    let arrow = tokens.iter().position(|token| token == "=>")?;
    let left = &tokens[..arrow];
    let right = &tokens[arrow + 1..];
    if !(left.len() == 1 || left.len() == 2) || !(right.len() == 1 || right.len() == 2) {
        return None;
    }

    Some(GoReplacementDirective {
        target_name: left[0].clone(),
        version_requirement: left.get(1).cloned(),
        replacement_target: right[0].clone(),
        replacement_version: right.get(1).cloned(),
        kind: RequirementKind::Replace,
        source_path: relative_path.to_string(),
        source_label,
        precision: TopologyPrecision::ExactStatic,
        status: TopologyStatus::Present,
    })
}

pub(super) fn unsupported_directive(
    relative_path: &str,
    source_label: &'static str,
    directive: &str,
) -> GoUnsupportedDirective {
    GoUnsupportedDirective {
        directive: directive.to_string(),
        source_path: relative_path.to_string(),
        source_label,
        precision: TopologyPrecision::Unknown,
        status: TopologyStatus::Unsupported,
    }
}

pub(super) fn go_tokens(entry: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = entry.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '"' {
            let mut end = None;
            let mut escaped = false;
            for (index, next) in chars.by_ref() {
                if next == '"' && !escaped {
                    end = Some(index + next.len_utf8());
                    break;
                }
                escaped = next == '\\' && !escaped;
                if next != '\\' {
                    escaped = false;
                }
            }
            let Some(end) = end else {
                return tokens;
            };
            if let Ok(value) = serde_json::from_str::<String>(&entry[start..end]) {
                tokens.push(value);
            }
            continue;
        }
        if ch == '`' {
            let Some((end, _)) = chars.by_ref().find(|(_, next)| *next == '`') else {
                return tokens;
            };
            tokens.push(entry[start + ch.len_utf8()..end].to_string());
            continue;
        }

        let end = chars
            .clone()
            .find(|(_, next)| next.is_whitespace())
            .map(|(index, _)| index)
            .unwrap_or(entry.len());
        while chars.peek().is_some_and(|(index, _)| *index < end) {
            chars.next();
        }
        tokens.push(entry[start..end].to_string());
    }
    tokens
}

fn split_directive(line: &str) -> Option<(&str, &str)> {
    let end = line
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace() || *ch == '(')
        .map(|(index, _)| index)
        .unwrap_or(line.len());
    let directive = &line[..end];
    if directive.is_empty() {
        return None;
    }
    Some((directive, &line[end..]))
}

fn split_block_entry(line: &str) -> (&str, bool) {
    if let Some((before, _)) = line.split_once(')') {
        (before.trim(), true)
    } else {
        (line.trim(), false)
    }
}

fn single_token(entry: &str) -> Option<String> {
    let tokens = go_tokens(entry);
    (tokens.len() == 1).then(|| tokens[0].clone())
}

fn strip_go_line_comment(line: &str) -> &str {
    line.split_once("//")
        .map(|(before, _)| before)
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polint_analysis::module_graph::topology::{
        RequirementKind, TopologyPrecision, TopologyStatus,
    };

    #[test]
    fn parse_go_mod_reads_module_go_requires_replaces_and_excludes() {
        let manifest = parse_go_mod(
            "services/api/go.mod",
            r#"
module example.com/api

go 1.24

require github.com/acme/direct v1.2.3
require (
    github.com/acme/block v0.4.0
)

replace github.com/acme/direct => ../direct
replace (
    github.com/acme/block v0.4.0 => github.com/acme/fork v0.4.1
)

exclude github.com/acme/bad v1.0.0
"#,
        );

        assert_eq!(manifest.module_path.as_deref(), Some("example.com/api"));
        assert_eq!(manifest.go_version.as_deref(), Some("1.24"));
        assert_eq!(
            manifest
                .requirements
                .iter()
                .map(|requirement| (
                    requirement.target_name.as_str(),
                    requirement.version_requirement.as_deref(),
                    requirement.kind,
                    requirement.source_label,
                    requirement.precision,
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "github.com/acme/direct",
                    Some("v1.2.3"),
                    RequirementKind::Direct,
                    "go.mod",
                    TopologyPrecision::ExactStatic,
                ),
                (
                    "github.com/acme/block",
                    Some("v0.4.0"),
                    RequirementKind::Direct,
                    "go.mod",
                    TopologyPrecision::ExactStatic,
                ),
            ]
        );
        assert_eq!(manifest.replacements.len(), 2);
        assert_eq!(manifest.replacements[0].kind, RequirementKind::Replace);
        assert_eq!(manifest.excludes[0].kind, RequirementKind::Exclude);
    }

    #[test]
    fn parse_go_mod_records_malformed_directives_as_unsupported() {
        let manifest = parse_go_mod(
            "go.mod",
            r#"
module
require github.com/acme/missing-version
replace github.com/acme/old =>
exclude github.com/acme/bad
"#,
        );

        assert_eq!(manifest.unsupported.len(), 4);
        assert!(
            manifest
                .unsupported
                .iter()
                .all(|row| row.status == TopologyStatus::Unsupported
                    && row.precision == TopologyPrecision::Unknown)
        );
    }
}
