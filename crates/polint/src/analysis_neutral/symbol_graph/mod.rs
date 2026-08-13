//! Language-neutral symbol graph models, identity, queries, and semantic closure.
//!
//! Concrete syntax and semantic adaptation belongs to the language frontend crates.
//! This module only owns normalized facts, stable identity algorithms, and composition
//! contracts consumed by those frontends.

pub mod model;
pub mod query;
pub mod semantic;
pub mod stable_id;

use crate::internal_core::{Diagnostic, Language};

/// Capabilities produced by the symbol graph providers.
pub const SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];

/// Normalized capability request passed from the facade composition root to a frontend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SymbolGraphRequest {
    pub symbols: bool,
    pub references: bool,
}

impl SymbolGraphRequest {
    pub const fn new(symbols: bool, references: bool) -> Self {
        Self {
            symbols,
            references,
        }
    }

    pub const fn any(self) -> bool {
        self.symbols || self.references
    }
}

/// Provider status returned by a concrete language adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolCapabilityStatus {
    Supported,
    SetupMissing,
    Unsupported,
}

/// Capability status with no facade rule-planning state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCapabilitySupport {
    pub capability: String,
    pub language: Language,
    pub status: SymbolCapabilityStatus,
    pub reason: Option<String>,
    pub hint: Option<String>,
}

/// Normalized output produced by one language adapter.
#[derive(Debug, Clone, Default)]
pub struct LanguageSymbolOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub capability_support: Vec<LanguageCapabilitySupport>,
    pub semantic: semantic::SemanticIndexOutput,
}
