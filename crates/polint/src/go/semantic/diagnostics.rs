#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoSemanticDiagnosticCategory {
    PackagesLoadFailed,
    VersionUnsupported,
    SidecarTimeout,
}

impl GoSemanticDiagnosticCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PackagesLoadFailed => GO_PACKAGES_LOAD_FAILED,
            Self::VersionUnsupported => GO_VERSION_UNSUPPORTED,
            Self::SidecarTimeout => GO_SIDECAR_TIMEOUT,
        }
    }
}

pub const GO_PACKAGES_LOAD_FAILED: &str = "GoPackagesLoadFailed";
pub const GO_VERSION_UNSUPPORTED: &str = "GoVersionUnsupported";
pub const GO_SIDECAR_TIMEOUT: &str = "GoSidecarTimeout";

pub fn category_for_package_error() -> GoSemanticDiagnosticCategory {
    GoSemanticDiagnosticCategory::PackagesLoadFailed
}

pub fn category_for_unsupported_go_version() -> GoSemanticDiagnosticCategory {
    GoSemanticDiagnosticCategory::VersionUnsupported
}

pub fn category_for_timeout() -> GoSemanticDiagnosticCategory {
    GoSemanticDiagnosticCategory::SidecarTimeout
}
