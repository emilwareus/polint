use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Language {
    Go,
    TypeScript,
    Tsx,
    JavaScript,
    Jsx,
    Unknown,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default()
        {
            "go" => Self::Go,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "js" | "mjs" | "cjs" => Self::JavaScript,
            "jsx" => Self::Jsx,
            _ => Self::Unknown,
        }
    }

    pub fn is_ts_family(self) -> bool {
        matches!(
            self,
            Self::TypeScript | Self::Tsx | Self::JavaScript | Self::Jsx
        )
    }
}
