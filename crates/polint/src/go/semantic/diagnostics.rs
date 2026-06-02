#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GoSemanticDiagnosticCategory {
    PackagesLoadFailed,
    VersionUnsupported,
    SidecarTimeout,
}

impl GoSemanticDiagnosticCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PackagesLoadFailed => GO_PACKAGES_LOAD_FAILED,
            Self::VersionUnsupported => GO_VERSION_UNSUPPORTED,
            Self::SidecarTimeout => GO_SIDECAR_TIMEOUT,
        }
    }
}

pub(crate) const GO_PACKAGES_LOAD_FAILED: &str = "GoPackagesLoadFailed";
pub(crate) const GO_VERSION_UNSUPPORTED: &str = "GoVersionUnsupported";
pub(crate) const GO_SIDECAR_TIMEOUT: &str = "GoSidecarTimeout";

pub(crate) fn category_for_package_error() -> GoSemanticDiagnosticCategory {
    GoSemanticDiagnosticCategory::PackagesLoadFailed
}

pub(crate) fn category_for_unsupported_go_version() -> GoSemanticDiagnosticCategory {
    GoSemanticDiagnosticCategory::VersionUnsupported
}

pub(crate) fn category_for_timeout() -> GoSemanticDiagnosticCategory {
    GoSemanticDiagnosticCategory::SidecarTimeout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AnalysisDb;

    #[test]
    fn go_04_category_strings_are_exact() {
        assert_eq!(
            category_for_package_error().as_str(),
            "GoPackagesLoadFailed"
        );
        assert_eq!(
            category_for_unsupported_go_version().as_str(),
            "GoVersionUnsupported"
        );
        assert_eq!(category_for_timeout().as_str(), "GoSidecarTimeout");
    }

    #[test]
    fn setup_missing_stores_zero_placeholder_go_semantic_facts() {
        let db = AnalysisDb::new();
        assert!(db.go_semantic_packages().is_empty());
        assert!(db.go_semantic_functions().is_empty());
        assert!(db.go_semantic_callsites().is_empty());
    }
}
