use crate::module_graph::topology::{RequirementKind, TopologyPrecision, TopologyStatus};

pub(crate) const PACKAGE_JSON_SOURCE_LABEL: &str = "package.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageJsonManifest {
    pub(crate) relative_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) name: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) package_manager: Option<String>,
    pub(crate) workspaces: Vec<String>,
    pub(crate) exports: Vec<String>,
    pub(crate) imports: Vec<String>,
    pub(crate) dependencies: Vec<PackageJsonDependency>,
    pub(crate) unsupported: Vec<PackageJsonUnsupported>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageJsonDependency {
    pub(crate) target_name: String,
    pub(crate) version_requirement: Option<String>,
    pub(crate) kind: RequirementKind,
    pub(crate) section: &'static str,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageJsonUnsupported {
    pub(crate) reason: String,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

pub(crate) fn parse_package_json(relative_path: &str, _contents: &str) -> PackageJsonManifest {
    PackageJsonManifest {
        relative_path: relative_path.to_string(),
        source_label: PACKAGE_JSON_SOURCE_LABEL,
        name: None,
        version: None,
        package_manager: None,
        workspaces: Vec::new(),
        exports: Vec::new(),
        imports: Vec::new(),
        dependencies: Vec::new(),
        unsupported: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_package_json_reads_package_workspace_exports_imports_and_dependency_sections() {
        let manifest = parse_package_json(
            "package.json",
            r##"{
  "name": "@acme/root",
  "version": "1.2.3",
  "packageManager": "pnpm@9.0.0",
  "workspaces": { "packages": ["packages/*", "tools/*"] },
  "exports": { ".": "./src/index.ts", "./feature": "./src/feature.ts" },
  "imports": { "#internal/*": "./src/internal/*" },
  "dependencies": { "react": "^18.0.0" },
  "devDependencies": { "vitest": "^2.0.0" },
  "peerDependencies": { "typescript": "^5.0.0" },
  "optionalDependencies": { "fsevents": "^2.0.0" },
  "bundleDependencies": ["left-pad"]
}"##,
        );

        assert_eq!(manifest.name.as_deref(), Some("@acme/root"));
        assert_eq!(manifest.version.as_deref(), Some("1.2.3"));
        assert_eq!(manifest.package_manager.as_deref(), Some("pnpm@9.0.0"));
        assert_eq!(manifest.workspaces, vec!["packages/*", "tools/*"]);
        assert_eq!(
            manifest.exports,
            vec![".:\"./src/index.ts\"", "./feature:\"./src/feature.ts\""]
        );
        assert_eq!(manifest.imports, vec!["#internal/*:\"./src/internal/*\""]);
        assert_eq!(
            manifest
                .dependencies
                .iter()
                .map(|dependency| (
                    dependency.section,
                    dependency.target_name.as_str(),
                    dependency.version_requirement.as_deref(),
                    dependency.kind,
                    dependency.precision,
                    dependency.status,
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "dependencies",
                    "react",
                    Some("^18.0.0"),
                    RequirementKind::Direct,
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
                (
                    "devDependencies",
                    "vitest",
                    Some("^2.0.0"),
                    RequirementKind::Development,
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
                (
                    "peerDependencies",
                    "typescript",
                    Some("^5.0.0"),
                    RequirementKind::Peer,
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
                (
                    "optionalDependencies",
                    "fsevents",
                    Some("^2.0.0"),
                    RequirementKind::Optional,
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
                (
                    "bundleDependencies",
                    "left-pad",
                    None,
                    RequirementKind::Build,
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
            ]
        );
    }

    #[test]
    fn parse_package_json_records_malformed_json_as_unsupported() {
        let manifest = parse_package_json("package.json", "{ this is not json");

        assert_eq!(manifest.unsupported.len(), 1);
        assert_eq!(manifest.unsupported[0].status, TopologyStatus::Unsupported);
        assert_eq!(manifest.unsupported[0].precision, TopologyPrecision::Unknown);
    }
}
