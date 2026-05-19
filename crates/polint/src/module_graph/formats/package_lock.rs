use crate::module_graph::topology::{TopologyPrecision, TopologyStatus};
use serde::Deserialize;
use std::collections::BTreeMap;

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

pub(crate) fn parse_package_lock(relative_path: &str, contents: &str) -> PackageLockManifest {
    let mut manifest = empty_manifest(relative_path);
    let Ok(lock) = serde_json::from_str::<PackageLockWire>(contents) else {
        manifest
            .unsupported
            .push(unsupported(relative_path, "malformed json"));
        return manifest;
    };

    manifest.lockfile_version = lock.lockfile_version;
    manifest.schema_label = schema_label(lock.lockfile_version);
    if lock.lockfile_version == Some(1) {
        manifest.unsupported.push(unsupported(
            relative_path,
            "package-lock v1 dependency tree is not supported",
        ));
        return manifest;
    }
    if lock
        .lockfile_version
        .is_some_and(|version| !matches!(version, 2 | 3))
    {
        manifest.unsupported.push(unsupported(
            relative_path,
            "unsupported package-lock version",
        ));
        return manifest;
    }
    manifest.packages = lock
        .packages
        .into_iter()
        .map(|(path, package)| package.into_manifest_package(relative_path, path))
        .collect();
    manifest
}

fn empty_manifest(relative_path: &str) -> PackageLockManifest {
    PackageLockManifest {
        relative_path: relative_path.to_string(),
        source_label: PACKAGE_LOCK_SOURCE_LABEL,
        lockfile_version: None,
        schema_label: "package-lock-unknown",
        packages: Vec::new(),
        unsupported: Vec::new(),
    }
}

fn schema_label(lockfile_version: Option<u64>) -> &'static str {
    match lockfile_version {
        Some(1) => "package-lock-v1",
        Some(2) => "package-lock-v2",
        Some(3) => "package-lock-v3",
        _ => "package-lock-unknown",
    }
}

fn unsupported(relative_path: &str, reason: &str) -> PackageLockUnsupported {
    PackageLockUnsupported {
        reason: reason.to_string(),
        source_path: relative_path.to_string(),
        source_label: PACKAGE_LOCK_SOURCE_LABEL,
        precision: TopologyPrecision::Unsupported,
        status: TopologyStatus::Unsupported,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageLockWire {
    lockfile_version: Option<u64>,
    #[serde(default)]
    packages: BTreeMap<String, PackageLockPackageWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageLockPackageWire {
    name: Option<String>,
    version: Option<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default)]
    dev: bool,
    #[serde(default)]
    optional: bool,
}

impl PackageLockPackageWire {
    fn into_manifest_package(self, relative_path: &str, path: String) -> PackageLockPackage {
        let name = self.name.or_else(|| package_name_from_lock_path(&path));
        PackageLockPackage {
            path,
            name,
            version: self.version,
            dependencies: self.dependencies.into_iter().collect(),
            dev: self.dev,
            optional: self.optional,
            source_path: relative_path.to_string(),
            source_label: PACKAGE_LOCK_SOURCE_LABEL,
            precision: TopologyPrecision::ExactLockfile,
            status: TopologyStatus::Resolved,
        }
    }
}

fn package_name_from_lock_path(path: &str) -> Option<String> {
    let suffix = path.strip_prefix("node_modules/")?;
    let mut parts = suffix.split('/');
    let first = parts.next()?;
    if first.starts_with('@') {
        let second = parts.next()?;
        Some(format!("{first}/{second}"))
    } else {
        Some(first.to_string())
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

    #[test]
    fn parse_package_lock_marks_malformed_json_unsupported() {
        let manifest = parse_package_lock("package-lock.json", "{");

        assert_eq!(manifest.packages, Vec::new());
        assert_eq!(manifest.unsupported.len(), 1);
        assert_eq!(manifest.unsupported[0].reason, "malformed json");
        assert_eq!(
            manifest.unsupported[0].precision,
            TopologyPrecision::Unsupported
        );
        assert_eq!(manifest.unsupported[0].status, TopologyStatus::Unsupported);
    }

    #[test]
    fn parse_package_lock_marks_v1_unsupported() {
        let manifest = parse_package_lock(
            "package-lock.json",
            r#"{
  "lockfileVersion": 1,
  "dependencies": {
    "react": { "version": "18.2.0" }
  }
}"#,
        );

        assert_eq!(manifest.lockfile_version, Some(1));
        assert_eq!(manifest.schema_label, "package-lock-v1");
        assert_eq!(manifest.packages, Vec::new());
        assert_eq!(manifest.unsupported.len(), 1);
        assert_eq!(
            manifest.unsupported[0].reason,
            "package-lock v1 dependency tree is not supported"
        );
    }
}
