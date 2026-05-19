#[cfg(test)]
mod tests {
    use super::*;
    use crate::module_graph::topology::{
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
