#[cfg(test)]
mod tests {
    use super::*;
    use crate::module_graph::topology::{
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
                (
                    "./services/api",
                    "go.work",
                    TopologyPrecision::ExactStatic,
                ),
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
