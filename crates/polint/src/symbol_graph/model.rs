use crate::core::{
    DefinitionFact, DefinitionId, DefinitionKind, FileId, Language, ModuleNodeId, PackageId,
    ReferenceFact, ReferenceId, ReferenceKind, Span, SymbolFact, SymbolId, SymbolKind,
    SymbolNamespace, SymbolPrecision, SymbolResolutionStatus,
};
use crate::diagnostics::{Diagnostic, TextRange};
use crate::symbol_graph::semantic::SemanticIndexOutput;
use crate::symbol_graph::stable_id::{
    StableDefinitionKey, StableKeyHash, StableReferenceKey, StableSymbolKey,
    default_stable_key_hash, definition_id_from_key_with_hash, reference_id_from_key_with_hash,
    symbol_id_from_key_with_hash,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SYMBOL_GRAPH_LAYER_SCHEMA: &str = "symbol-graph-facts-3";

#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolGraphOutput {
    pub(crate) symbols: Vec<SymbolFact>,
    pub(crate) definitions: Vec<DefinitionFact>,
    pub(crate) references: Vec<ReferenceFact>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SymbolGraphLayerPayload {
    pub(crate) schema: String,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<crate::core::CapabilitySupport>,
    pub(crate) symbols: Vec<SymbolFact>,
    pub(crate) definitions: Vec<DefinitionFact>,
    pub(crate) references: Vec<ReferenceFact>,
    pub(crate) semantic_index: SemanticIndexOutput,
}

#[derive(Debug, Clone)]
pub(crate) struct SymbolGraphBuilder {
    hash: StableKeyHash,
    symbols: BTreeMap<String, SymbolFact>,
    definitions: BTreeMap<String, DefinitionFact>,
    references: BTreeMap<String, ReferenceFact>,
    symbol_keys_by_id: BTreeMap<SymbolId, StableSymbolKey>,
    id_keys: BTreeMap<IdFamilyKey, BTreeSet<String>>,
    diagnostics: BTreeMap<String, Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolDraft {
    pub(crate) language: Language,
    pub(crate) name: String,
    pub(crate) qualified_name: String,
    pub(crate) kind: SymbolKind,
    pub(crate) namespace: SymbolNamespace,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) owner: Option<SymbolId>,
    pub(crate) module_key: Option<String>,
    pub(crate) package_key: Option<String>,
    pub(crate) file_key: Option<String>,
    pub(crate) owner_chain: Vec<String>,
    pub(crate) primary_span: Option<Span>,
    pub(crate) is_exported: bool,
    pub(crate) precision: SymbolPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefinitionDraft {
    pub(crate) language: Language,
    pub(crate) name: String,
    pub(crate) qualified_name: String,
    pub(crate) kind: DefinitionKind,
    pub(crate) namespace: SymbolNamespace,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) owner: Option<SymbolId>,
    pub(crate) file_key: String,
    pub(crate) primary_span: Option<Span>,
    pub(crate) is_primary: bool,
    pub(crate) is_exported: bool,
    pub(crate) precision: SymbolPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceDraft {
    pub(crate) language: Language,
    pub(crate) name: String,
    pub(crate) qualified_name: String,
    pub(crate) kind: ReferenceKind,
    pub(crate) namespace: SymbolNamespace,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) owner: Option<SymbolId>,
    pub(crate) file_key: String,
    pub(crate) primary_span: Option<Span>,
    pub(crate) precision: SymbolPrecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum IdFamily {
    Symbol,
    Definition,
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct IdFamilyKey {
    family: IdFamily,
    id: u64,
}

impl SymbolGraphBuilder {
    pub(crate) fn new() -> Self {
        Self::with_hash(default_stable_key_hash)
    }

    #[cfg(test)]
    pub(crate) fn with_hash_for_test(hash: StableKeyHash) -> Self {
        Self::with_hash(hash)
    }

    fn with_hash(hash: StableKeyHash) -> Self {
        Self {
            hash,
            symbols: BTreeMap::new(),
            definitions: BTreeMap::new(),
            references: BTreeMap::new(),
            symbol_keys_by_id: BTreeMap::new(),
            id_keys: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
        }
    }

    pub(crate) fn add_symbol(&mut self, draft: SymbolDraft) -> SymbolId {
        let stable_key_input = draft.stable_key_input();
        let id = symbol_id_from_key_with_hash(&stable_key_input, self.hash);
        let stable_key = stable_key_input.stable_key();
        let fact = draft.into_fact(id, stable_key.clone());

        self.record_id_key(IdFamily::Symbol, id.0, &stable_key);
        self.symbol_keys_by_id
            .entry(id)
            .or_insert_with(|| stable_key_input.clone());
        self.insert_symbol(stable_key, fact);
        id
    }

    pub(crate) fn add_definition(
        &mut self,
        symbol: SymbolId,
        draft: DefinitionDraft,
    ) -> DefinitionId {
        let symbol_key = self
            .symbol_keys_by_id
            .get(&symbol)
            .cloned()
            .unwrap_or_else(|| draft.fallback_symbol_key());
        let span = draft
            .primary_span
            .clone()
            .unwrap_or_else(|| fallback_span(draft.file));
        let stable_key_input = StableDefinitionKey::new(symbol_key, draft.file_key.clone(), span);
        let id = definition_id_from_key_with_hash(&stable_key_input, self.hash);
        let stable_key = stable_key_input.stable_key();
        let fact = draft.into_fact(id, symbol, stable_key.clone());

        self.record_id_key(IdFamily::Definition, id.0, &stable_key);
        self.insert_definition(stable_key, fact);
        id
    }

    pub(crate) fn add_reference(&mut self, target: SymbolId, draft: ReferenceDraft) -> ReferenceId {
        self.add_reference_with_status(
            Some(target),
            Vec::new(),
            SymbolResolutionStatus::Resolved,
            draft,
        )
    }

    pub(crate) fn add_ambiguous_reference(
        &mut self,
        mut candidates: Vec<SymbolId>,
        draft: ReferenceDraft,
    ) -> ReferenceId {
        candidates.sort_unstable();
        candidates.dedup();
        self.add_reference_with_status(None, candidates, SymbolResolutionStatus::Ambiguous, draft)
    }

    pub(crate) fn add_unresolved_reference(&mut self, draft: ReferenceDraft) -> ReferenceId {
        self.add_reference_with_status(None, Vec::new(), SymbolResolutionStatus::Unresolved, draft)
    }

    pub(crate) fn add_setup_missing_reference(
        &mut self,
        language: Language,
        file: FileId,
        file_key: impl Into<String>,
        name: impl Into<String>,
    ) -> ReferenceId {
        let name = name.into();
        let draft = ReferenceDraft::status_row(
            language,
            file,
            file_key,
            name,
            SymbolPrecision::SetupMissing,
        );
        self.add_reference_with_status(
            None,
            Vec::new(),
            SymbolResolutionStatus::SetupMissing,
            draft,
        )
    }

    pub(crate) fn add_setup_missing_reference_draft(
        &mut self,
        mut draft: ReferenceDraft,
    ) -> ReferenceId {
        draft.precision = SymbolPrecision::SetupMissing;
        self.add_reference_with_status(
            None,
            Vec::new(),
            SymbolResolutionStatus::SetupMissing,
            draft,
        )
    }

    #[cfg(test)]
    pub(crate) fn add_unsupported_reference(
        &mut self,
        language: Language,
        file: FileId,
        file_key: impl Into<String>,
        name: impl Into<String>,
    ) -> ReferenceId {
        let name = name.into();
        let draft = ReferenceDraft::status_row(
            language,
            file,
            file_key,
            name,
            SymbolPrecision::Unsupported,
        );
        self.add_reference_with_status(None, Vec::new(), SymbolResolutionStatus::Unsupported, draft)
    }

    pub(crate) fn add_unsupported_reference_draft(
        &mut self,
        mut draft: ReferenceDraft,
    ) -> ReferenceId {
        draft.precision = SymbolPrecision::Unsupported;
        self.add_reference_with_status(None, Vec::new(), SymbolResolutionStatus::Unsupported, draft)
    }

    pub(crate) fn finish(self) -> SymbolGraphOutput {
        let mut symbols = self.symbols.into_values().collect::<Vec<_>>();
        let mut definitions = self.definitions.into_values().collect::<Vec<_>>();
        let mut references = self.references.into_values().collect::<Vec<_>>();
        symbols.sort_by(symbol_order);
        definitions.sort_by(definition_order);
        references.sort_by(reference_order);

        let mut diagnostics = self.diagnostics.into_values().collect::<Vec<_>>();
        diagnostics.sort_by(|left, right| {
            (
                left.rule_id.as_str(),
                left.file.as_str(),
                left.range.start_line,
                left.range.start_col,
                left.message.as_str(),
                left.stable_fingerprint.as_str(),
            )
                .cmp(&(
                    right.rule_id.as_str(),
                    right.file.as_str(),
                    right.range.start_line,
                    right.range.start_col,
                    right.message.as_str(),
                    right.stable_fingerprint.as_str(),
                ))
        });

        SymbolGraphOutput {
            symbols,
            definitions,
            references,
            diagnostics,
        }
    }

    fn add_reference_with_status(
        &mut self,
        target: Option<SymbolId>,
        candidates: Vec<SymbolId>,
        status: SymbolResolutionStatus,
        draft: ReferenceDraft,
    ) -> ReferenceId {
        let span = draft
            .primary_span
            .clone()
            .unwrap_or_else(|| fallback_span(draft.file));
        let stable_key_input = target
            .and_then(|id| self.symbol_keys_by_id.get(&id).cloned())
            .map_or_else(
                || {
                    StableReferenceKey::unresolved(
                        draft.language,
                        draft.file_key.clone(),
                        draft.name.clone(),
                        span.clone(),
                    )
                },
                |target_key| {
                    StableReferenceKey::resolved(target_key, draft.file_key.clone(), span.clone())
                },
            );
        let id = reference_id_from_key_with_hash(&stable_key_input, self.hash);
        let stable_key = stable_key_input.stable_key();
        let fact = draft.into_fact(id, target, candidates, stable_key.clone(), status);

        self.record_id_key(IdFamily::Reference, id.0, &stable_key);
        self.insert_reference(stable_key, fact);
        id
    }

    fn insert_symbol(&mut self, stable_key: String, fact: SymbolFact) {
        if let Some(existing) = self.symbols.get(&stable_key) {
            if !same_symbol_fact(existing, &fact) {
                self.record_duplicate_diagnostic("symbol", fact.id.0, &stable_key);
            }
            return;
        }
        self.symbols.insert(stable_key, fact);
    }

    fn insert_definition(&mut self, stable_key: String, fact: DefinitionFact) {
        if let Some(existing) = self.definitions.get(&stable_key) {
            if !same_definition_fact(existing, &fact) {
                self.record_duplicate_diagnostic("definition", fact.id.0, &stable_key);
            }
            return;
        }
        self.definitions.insert(stable_key, fact);
    }

    fn insert_reference(&mut self, stable_key: String, fact: ReferenceFact) {
        if let Some(existing) = self.references.get(&stable_key) {
            if !same_reference_fact(existing, &fact) {
                self.record_duplicate_diagnostic("reference", fact.id.0, &stable_key);
            }
            return;
        }
        self.references.insert(stable_key, fact);
    }

    fn record_id_key(&mut self, family: IdFamily, id: u64, stable_key: &str) {
        let keys = self.id_keys.entry(IdFamilyKey { family, id }).or_default();
        keys.insert(stable_key.to_string());
        if keys.len() > 1 {
            let keys = keys.iter().cloned().collect::<Vec<_>>();
            self.record_collision_diagnostic(family, id, &keys);
        }
    }

    fn record_collision_diagnostic(&mut self, family: IdFamily, id: u64, keys: &[String]) {
        let key = format!("collision:{}:{id:016x}", family_label(family));
        self.diagnostics.entry(key).or_insert_with(|| {
            Diagnostic::error(
                "polint/internal",
                "<workspace>",
                TextRange::point(1, 1),
                format!(
                    "Symbol graph {} stable ID collision detected.",
                    family_label(family)
                ),
            )
            .with_evidence("id", format!("{id:016x}"))
            .with_evidence("stable_keys", keys.join("\n"))
        });
    }

    fn record_duplicate_diagnostic(&mut self, family: &str, id: u64, stable_key: &str) {
        let key = format!("duplicate:{family}:{id:016x}:{stable_key}");
        self.diagnostics.entry(key).or_insert_with(|| {
            Diagnostic::error(
                "polint/internal",
                "<workspace>",
                TextRange::point(1, 1),
                format!("Symbol graph duplicate {family} stable key has incompatible facts."),
            )
            .with_evidence("id", format!("{id:016x}"))
            .with_evidence("stable_key", stable_key.to_string())
        });
    }
}

impl SymbolDraft {
    fn stable_key_input(&self) -> StableSymbolKey {
        StableSymbolKey::new(
            self.language,
            self.module_key.clone(),
            self.package_key.clone(),
            self.file_key.clone(),
            self.owner_chain.clone(),
            self.namespace,
            self.kind,
            self.name.clone(),
            self.primary_span.clone(),
        )
    }

    fn into_fact(self, id: SymbolId, stable_key: String) -> SymbolFact {
        SymbolFact {
            id,
            language: self.language,
            name: self.name,
            qualified_name: self.qualified_name,
            kind: self.kind,
            namespace: self.namespace,
            file: self.file,
            package: self.package,
            module: self.module,
            owner: self.owner,
            primary_span: self.primary_span,
            is_exported: self.is_exported,
            stable_key,
            precision: self.precision,
        }
    }
}

impl DefinitionDraft {
    fn fallback_symbol_key(&self) -> StableSymbolKey {
        StableSymbolKey::new(
            self.language,
            None,
            None,
            Some(self.file_key.clone()),
            Vec::new(),
            self.namespace,
            SymbolKind::Unknown,
            self.name.clone(),
            self.primary_span.clone(),
        )
    }

    fn into_fact(self, id: DefinitionId, symbol: SymbolId, stable_key: String) -> DefinitionFact {
        DefinitionFact {
            id,
            symbol,
            language: self.language,
            name: self.name,
            qualified_name: self.qualified_name,
            kind: self.kind,
            namespace: self.namespace,
            file: self.file,
            package: self.package,
            module: self.module,
            owner: self.owner,
            primary_span: self.primary_span,
            is_primary: self.is_primary,
            is_exported: self.is_exported,
            stable_key,
            precision: self.precision,
        }
    }
}

impl ReferenceDraft {
    fn status_row(
        language: Language,
        file: FileId,
        file_key: impl Into<String>,
        name: String,
        precision: SymbolPrecision,
    ) -> Self {
        Self {
            language,
            qualified_name: name.clone(),
            name,
            kind: ReferenceKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            file_key: file_key.into(),
            primary_span: None,
            precision,
        }
    }

    fn into_fact(
        self,
        id: ReferenceId,
        target: Option<SymbolId>,
        candidates: Vec<SymbolId>,
        stable_key: String,
        status: SymbolResolutionStatus,
    ) -> ReferenceFact {
        ReferenceFact {
            id,
            language: self.language,
            name: self.name,
            qualified_name: self.qualified_name,
            kind: self.kind,
            namespace: self.namespace,
            file: self.file,
            package: self.package,
            module: self.module,
            owner: self.owner,
            primary_span: self.primary_span,
            target,
            candidates,
            stable_key,
            status,
            precision: self.precision,
        }
    }
}

fn fallback_span(file: Option<FileId>) -> Span {
    Span::point(file.unwrap_or(FileId(u32::MAX)), 1, 1)
}

fn same_symbol_fact(left: &SymbolFact, right: &SymbolFact) -> bool {
    left.id == right.id
        && left.language == right.language
        && left.name == right.name
        && left.qualified_name == right.qualified_name
        && left.kind == right.kind
        && left.namespace == right.namespace
        && left.file == right.file
        && left.package == right.package
        && left.module == right.module
        && left.owner == right.owner
        && left.primary_span == right.primary_span
        && left.is_exported == right.is_exported
        && left.precision == right.precision
}

fn same_definition_fact(left: &DefinitionFact, right: &DefinitionFact) -> bool {
    left.id == right.id
        && left.symbol == right.symbol
        && left.language == right.language
        && left.name == right.name
        && left.qualified_name == right.qualified_name
        && left.kind == right.kind
        && left.namespace == right.namespace
        && left.file == right.file
        && left.package == right.package
        && left.module == right.module
        && left.owner == right.owner
        && left.primary_span == right.primary_span
        && left.is_primary == right.is_primary
        && left.is_exported == right.is_exported
        && left.precision == right.precision
}

fn same_reference_fact(left: &ReferenceFact, right: &ReferenceFact) -> bool {
    left.id == right.id
        && left.language == right.language
        && left.name == right.name
        && left.qualified_name == right.qualified_name
        && left.kind == right.kind
        && left.namespace == right.namespace
        && left.file == right.file
        && left.package == right.package
        && left.module == right.module
        && left.owner == right.owner
        && left.primary_span == right.primary_span
        && left.target == right.target
        && left.candidates == right.candidates
        && left.status == right.status
        && left.precision == right.precision
}

fn symbol_order(left: &SymbolFact, right: &SymbolFact) -> std::cmp::Ordering {
    fact_order_key(
        &left.stable_key,
        left.file,
        left.primary_span.as_ref(),
        symbol_kind_rank(left.kind),
        &left.name,
    )
    .cmp(&fact_order_key(
        &right.stable_key,
        right.file,
        right.primary_span.as_ref(),
        symbol_kind_rank(right.kind),
        &right.name,
    ))
}

fn definition_order(left: &DefinitionFact, right: &DefinitionFact) -> std::cmp::Ordering {
    fact_order_key(
        &left.stable_key,
        left.file,
        left.primary_span.as_ref(),
        definition_kind_rank(left.kind),
        &left.name,
    )
    .cmp(&fact_order_key(
        &right.stable_key,
        right.file,
        right.primary_span.as_ref(),
        definition_kind_rank(right.kind),
        &right.name,
    ))
}

fn reference_order(left: &ReferenceFact, right: &ReferenceFact) -> std::cmp::Ordering {
    fact_order_key(
        &left.stable_key,
        left.file,
        left.primary_span.as_ref(),
        reference_kind_rank(left.kind),
        &left.name,
    )
    .cmp(&fact_order_key(
        &right.stable_key,
        right.file,
        right.primary_span.as_ref(),
        reference_kind_rank(right.kind),
        &right.name,
    ))
}

fn fact_order_key<'a>(
    stable_key: &'a str,
    file: Option<FileId>,
    span: Option<&Span>,
    kind: u8,
    name: &'a str,
) -> (Option<FileId>, u32, u32, u8, &'a str, &'a str) {
    let (start_byte, end_byte) = span
        .map(|span| (span.start_byte, span.end_byte))
        .unwrap_or((u32::MAX, u32::MAX));
    (file, start_byte, end_byte, kind, name, stable_key)
}

fn symbol_kind_rank(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Package => 0,
        SymbolKind::Module => 1,
        SymbolKind::File => 2,
        SymbolKind::Function => 3,
        SymbolKind::Method => 4,
        SymbolKind::Class => 5,
        SymbolKind::Interface => 6,
        SymbolKind::TypeAlias => 7,
        SymbolKind::Enum => 8,
        SymbolKind::EnumMember => 9,
        SymbolKind::Variable => 10,
        SymbolKind::Constant => 11,
        SymbolKind::Parameter => 12,
        SymbolKind::Field => 13,
        SymbolKind::Property => 14,
        SymbolKind::Namespace => 15,
        SymbolKind::Import => 16,
        SymbolKind::Export => 17,
        SymbolKind::Unknown => 18,
    }
}

fn definition_kind_rank(kind: DefinitionKind) -> u8 {
    match kind {
        DefinitionKind::Declaration => 0,
        DefinitionKind::Definition => 1,
        DefinitionKind::Import => 2,
        DefinitionKind::Export => 3,
        DefinitionKind::Implicit => 4,
        DefinitionKind::Unknown => 5,
    }
}

fn reference_kind_rank(kind: ReferenceKind) -> u8 {
    match kind {
        ReferenceKind::Read => 0,
        ReferenceKind::Write => 1,
        ReferenceKind::ReadWrite => 2,
        ReferenceKind::Call => 3,
        ReferenceKind::TypeUse => 4,
        ReferenceKind::Import => 5,
        ReferenceKind::Export => 6,
        ReferenceKind::MemberAccess => 7,
        ReferenceKind::Assignment => 8,
        ReferenceKind::DeclarationUse => 9,
        ReferenceKind::Unknown => 10,
    }
}

fn family_label(family: IdFamily) -> &'static str {
    match family {
        IdFamily::Symbol => "symbol",
        IdFamily::Definition => "definition",
        IdFamily::Reference => "reference",
    }
}

#[cfg(test)]
mod symbol_graph_builder {
    use crate::core::{
        DefinitionKind, FileId, Language, ReferenceKind, Span, SymbolKind, SymbolNamespace,
        SymbolPrecision, SymbolResolutionStatus,
    };
    use crate::symbol_graph::model::{
        DefinitionDraft, ReferenceDraft, SymbolDraft, SymbolGraphBuilder,
    };

    fn span(file: FileId, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 5,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: start_byte + 6,
        }
    }

    fn symbol_draft(name: &str, file: FileId, start_byte: u32) -> SymbolDraft {
        SymbolDraft {
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: SymbolKind::Function,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            module_key: Some("module:app".to_string()),
            package_key: Some("package:web".to_string()),
            file_key: Some("src/app.ts".to_string()),
            owner_chain: Vec::new(),
            primary_span: Some(span(file, start_byte)),
            is_exported: true,
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn definition_draft(name: &str, file: FileId, start_byte: u32) -> DefinitionDraft {
        DefinitionDraft {
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: DefinitionKind::Declaration,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            file_key: "src/app.ts".to_string(),
            primary_span: Some(span(file, start_byte)),
            is_primary: true,
            is_exported: true,
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn reference_draft(name: &str, file: FileId, start_byte: u32) -> ReferenceDraft {
        ReferenceDraft {
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: ReferenceKind::Read,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            file_key: "src/app.ts".to_string(),
            primary_span: Some(span(file, start_byte)),
            precision: SymbolPrecision::ExactLocal,
        }
    }

    #[test]
    fn output_order_is_deterministic_across_insertion_order() {
        let file = FileId(0);
        let mut left = SymbolGraphBuilder::new();
        let zeta = left.add_symbol(symbol_draft("zeta", file, 20));
        let alpha = left.add_symbol(symbol_draft("alpha", file, 10));
        left.add_definition(zeta, definition_draft("zeta", file, 20));
        left.add_definition(alpha, definition_draft("alpha", file, 10));
        left.add_reference(zeta, reference_draft("zeta", file, 80));
        left.add_reference(alpha, reference_draft("alpha", file, 70));

        let mut right = SymbolGraphBuilder::new();
        let alpha = right.add_symbol(symbol_draft("alpha", file, 10));
        let zeta = right.add_symbol(symbol_draft("zeta", file, 20));
        right.add_reference(alpha, reference_draft("alpha", file, 70));
        right.add_reference(zeta, reference_draft("zeta", file, 80));
        right.add_definition(alpha, definition_draft("alpha", file, 10));
        right.add_definition(zeta, definition_draft("zeta", file, 20));

        let left = left.finish();
        let right = right.finish();

        assert_eq!(
            left.symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        assert_eq!(
            left.symbols
                .iter()
                .map(|symbol| symbol.stable_key.as_str())
                .collect::<Vec<_>>(),
            right
                .symbols
                .iter()
                .map(|symbol| symbol.stable_key.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            left.references
                .iter()
                .map(|reference| reference.name.as_str())
                .collect::<Vec<_>>(),
            right
                .references
                .iter()
                .map(|reference| reference.name.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn id_collisions_emit_deterministic_internal_diagnostics() {
        fn constant_hash(_: &str) -> String {
            "0000000000000001".to_string()
        }

        let file = FileId(0);
        let mut builder = SymbolGraphBuilder::with_hash_for_test(constant_hash);
        let alpha = builder.add_symbol(symbol_draft("alpha", file, 10));
        let beta = builder.add_symbol(symbol_draft("beta", file, 20));
        let output = builder.finish();

        assert_eq!(alpha, beta);
        assert_eq!(
            output
                .symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
        assert_eq!(output.diagnostics[0].rule_id, "polint/internal");
        assert_eq!(output.diagnostics[0].evidence[0].label, "id");
    }

    #[test]
    fn precision_and_reference_status_are_preserved() {
        let file = FileId(0);
        let mut builder = SymbolGraphBuilder::new();
        let mut symbol = symbol_draft("alpha", file, 10);
        symbol.precision = SymbolPrecision::ExactSemantic;
        let alpha = builder.add_symbol(symbol);
        let mut definition = definition_draft("alpha", file, 10);
        definition.precision = SymbolPrecision::ModuleLinked;
        builder.add_definition(alpha, definition);
        builder.add_reference(alpha, reference_draft("alpha", file, 40));
        let mut ambiguous = reference_draft("ambiguous", file, 50);
        ambiguous.precision = SymbolPrecision::Ambiguous;
        builder.add_ambiguous_reference(vec![alpha], ambiguous);
        let mut unresolved = reference_draft("missing", file, 60);
        unresolved.precision = SymbolPrecision::Unresolved;
        builder.add_unresolved_reference(unresolved);
        builder.add_setup_missing_reference(Language::Go, file, "cmd/main.go", "go/types");
        builder.add_unsupported_reference(Language::Unknown, file, "README.md", "markdown");

        let output = builder.finish();

        assert_eq!(output.symbols[0].precision, SymbolPrecision::ExactSemantic);
        assert_eq!(
            output.definitions[0].precision,
            SymbolPrecision::ModuleLinked
        );
        assert_eq!(
            output
                .references
                .iter()
                .map(|reference| reference.status)
                .collect::<Vec<_>>(),
            vec![
                SymbolResolutionStatus::Resolved,
                SymbolResolutionStatus::Ambiguous,
                SymbolResolutionStatus::Unresolved,
                SymbolResolutionStatus::SetupMissing,
                SymbolResolutionStatus::Unsupported,
            ]
        );
    }
}
