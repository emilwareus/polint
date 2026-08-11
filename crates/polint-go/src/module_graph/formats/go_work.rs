use super::go_mod::{
    GoReplacementDirective, GoUnsupportedDirective, parse_directive_lines,
    parse_replacement_directive, unsupported_directive,
};
use polint_analysis::module_graph::topology::{TopologyPrecision, TopologyStatus};

pub(crate) const GO_WORK_SOURCE_LABEL: &str = "go.work";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoWorkManifest {
    pub(crate) relative_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) go_version: Option<String>,
    pub(crate) use_roots: Vec<GoWorkUseDirective>,
    pub(crate) replacements: Vec<GoReplacementDirective>,
    pub(crate) unsupported: Vec<GoUnsupportedDirective>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoWorkUseDirective {
    pub(crate) path: String,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

pub(crate) fn parse_go_work(relative_path: &str, contents: &str) -> GoWorkManifest {
    let mut manifest = GoWorkManifest {
        relative_path: relative_path.to_string(),
        source_label: GO_WORK_SOURCE_LABEL,
        go_version: None,
        use_roots: Vec::new(),
        replacements: Vec::new(),
        unsupported: Vec::new(),
    };

    parse_directive_lines(contents, |directive, entry| {
        apply_go_work_directive(&mut manifest, relative_path, directive, entry);
    });

    manifest
}

fn apply_go_work_directive(
    manifest: &mut GoWorkManifest,
    relative_path: &str,
    directive: &str,
    entry: &str,
) {
    match directive {
        "go" => {
            let tokens = super::go_mod::go_tokens(entry);
            if tokens.len() == 1 {
                manifest.go_version = Some(tokens[0].clone());
            } else {
                manifest.unsupported.push(unsupported_directive(
                    relative_path,
                    GO_WORK_SOURCE_LABEL,
                    directive,
                ));
            }
        }
        "use" => {
            let tokens = super::go_mod::go_tokens(entry);
            if tokens.len() == 1 {
                manifest.use_roots.push(GoWorkUseDirective {
                    path: tokens[0].clone(),
                    source_path: relative_path.to_string(),
                    source_label: GO_WORK_SOURCE_LABEL,
                    precision: TopologyPrecision::ExactStatic,
                    status: TopologyStatus::Present,
                });
            } else {
                manifest.unsupported.push(unsupported_directive(
                    relative_path,
                    GO_WORK_SOURCE_LABEL,
                    directive,
                ));
            }
        }
        "replace" => {
            match parse_replacement_directive(relative_path, GO_WORK_SOURCE_LABEL, entry) {
                Some(replacement) => manifest.replacements.push(replacement),
                None => manifest.unsupported.push(unsupported_directive(
                    relative_path,
                    GO_WORK_SOURCE_LABEL,
                    directive,
                )),
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polint_analysis::module_graph::topology::{
        RequirementKind, TopologyPrecision, TopologyStatus,
    };

    #[test]
    fn parse_go_work_reads_use_roots_and_replaces() {
        let manifest = parse_go_work(
            "go.work",
            r#"
go 1.24

use ./services/api
use (
    ./libs/common
)

replace github.com/acme/lib => ../lib
replace (
    github.com/acme/other v1.0.0 => github.com/acme/fork v1.0.1
)
"#,
        );

        assert_eq!(
            manifest
                .use_roots
                .iter()
                .map(|root| (root.path.as_str(), root.source_label, root.precision))
                .collect::<Vec<_>>(),
            vec![
                ("./services/api", "go.work", TopologyPrecision::ExactStatic,),
                ("./libs/common", "go.work", TopologyPrecision::ExactStatic),
            ]
        );
        assert_eq!(manifest.replacements.len(), 2);
        assert!(
            manifest
                .replacements
                .iter()
                .all(|replacement| replacement.kind == RequirementKind::Replace)
        );
    }

    #[test]
    fn parse_go_work_records_malformed_directives_as_unsupported() {
        let manifest = parse_go_work(
            "go.work",
            r#"
use
replace github.com/acme/old =>
"#,
        );

        assert_eq!(manifest.unsupported.len(), 2);
        assert!(
            manifest
                .unsupported
                .iter()
                .all(|row| row.status == TopologyStatus::Unsupported
                    && row.precision == TopologyPrecision::Unknown)
        );
    }
}
