use crate::module_graph::topology::{TopologyPrecision, TopologyStatus};

pub(crate) const PACKAGE_LOCK_SOURCE_LABEL: &str = "package-lock.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageLockManifest {
    pub(crate) relative_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) lockfile_version: Option<u64>,
    pub(crate) schema_label: &'static str,
    pub(crate) packages: Vec<PackageLockPackage>,
    pub(crate) unsupported: Vec<PackageLockUnsupported>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageLockPackage {
    pub(crate) path: String,
    pub(crate) name: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) dependencies: Vec<(String, String)>,
    pub(crate) dev: bool,
    pub(crate) optional: bool,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageLockUnsupported {
    pub(crate) reason: String,
    pub(crate) source_path: String,
    pub(crate) source_label: &'static str,
    pub(crate) precision: TopologyPrecision,
    pub(crate) status: TopologyStatus,
}

pub(crate) fn parse_package_lock(relative_path: &str, _contents: &str) -> PackageLockManifest {
    PackageLockManifest {
        relative_path: relative_path.to_string(),
        source_label: PACKAGE_LOCK_SOURCE_LABEL,
        lockfile_version: None,
        schema_label: "package-lock-unknown",
        packages: Vec::new(),
        unsupported: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_package_lock_reads_v3_package_selection_evidence() {
        let manifest = parse_package_lock(
            "package-lock.json",
            r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "@acme/root", "version": "1.0.0", "dependencies": { "react": "^18.0.0" } },
    "node_modules/react": { "version": "18.2.0", "dev": true, "optional": true }
  }
}"#,
        );

        assert_eq!(manifest.lockfile_version, Some(3));
        assert_eq!(manifest.schema_label, "package-lock-v3");
        assert_eq!(manifest.packages.len(), 2);
        assert_eq!(manifest.packages[0].path, "");
        assert_eq!(manifest.packages[0].name.as_deref(), Some("@acme/root"));
        assert_eq!(
            manifest.packages[0].dependencies,
            vec![("react".to_string(), "^18.0.0".to_string())]
        );
        assert_eq!(manifest.packages[1].path, "node_modules/react");
        assert_eq!(manifest.packages[1].version.as_deref(), Some("18.2.0"));
        assert!(manifest.packages[1].dev);
        assert!(manifest.packages[1].optional);
    }
}
