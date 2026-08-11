use super::stable_id::{
    StableDefinitionKey, StableKeyHash, StableReferenceKey, StableSymbolKey,
    default_stable_key_hash, definition_id_from_key_with_hash, reference_id_from_key_with_hash,
    symbol_id_from_key_with_hash,
};
use polint_analysis_api::{
    DefinitionFact, DefinitionKind, ReferenceFact, ReferenceKind, SymbolFact, SymbolKind,
    SymbolNamespace, SymbolPrecision, SymbolResolutionStatus,
};
use polint_core::{
    DefinitionId, FileId, Language, ModuleNodeId, PackageId, ReferenceId, Span, StableKeyId,
    StableKeyInterner, SymbolId,
};
use polint_core::{Diagnostic, DiagnosticRange as TextRange};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct SymbolGraphOutput {
    pub symbols: Vec<SymbolFact>,
    pub definitions: Vec<DefinitionFact>,
    pub references: Vec<ReferenceFact>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct SymbolGraphBuilder {
    interner: StableKeyInterner,
    hash: StableKeyHash,
    symbols: BTreeMap<String, SymbolFact>,
    definitions: BTreeMap<String, DefinitionFact>,
    references: BTreeMap<String, ReferenceFact>,
    symbol_keys_by_id: BTreeMap<SymbolId, StableSymbolKey>,
    id_keys: BTreeMap<IdFamilyKey, BTreeSet<String>>,
    diagnostics: BTreeMap<String, Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolDraft {
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub module_key: Option<String>,
    pub package_key: Option<String>,
    pub file_key: Option<String>,
    pub owner_chain: Vec<String>,
    pub primary_span: Option<Span>,
    pub is_exported: bool,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionDraft {
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: DefinitionKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub file_key: String,
    pub primary_span: Option<Span>,
    pub is_primary: bool,
    pub is_exported: bool,
    pub precision: SymbolPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDraft {
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: ReferenceKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub file_key: String,
    pub primary_span: Option<Span>,
    pub precision: SymbolPrecision,
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
    pub fn new(interner: StableKeyInterner) -> Self {
        Self::with_hash(interner, default_stable_key_hash)
    }

    #[cfg(test)]
    pub fn with_hash_for_test(interner: StableKeyInterner, hash: StableKeyHash) -> Self {
        Self::with_hash(interner, hash)
    }

    fn with_hash(interner: StableKeyInterner, hash: StableKeyHash) -> Self {
        Self {
            interner,
            hash,
            symbols: BTreeMap::new(),
            definitions: BTreeMap::new(),
            references: BTreeMap::new(),
            symbol_keys_by_id: BTreeMap::new(),
            id_keys: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
        }
    }

    pub fn add_symbol(&mut self, draft: SymbolDraft) -> SymbolId {
        let stable_key_input = draft.stable_key_input();
        let id = symbol_id_from_key_with_hash(&stable_key_input, self.hash);
        let stable_key_text = stable_key_input.stable_key();
        let fact = draft.into_fact(id, self.interner.intern(stable_key_text.clone()));

        self.record_id_key(IdFamily::Symbol, id.0, &stable_key_text);
        self.symbol_keys_by_id
            .entry(id)
            .or_insert_with(|| stable_key_input.clone());
        self.insert_symbol(stable_key_text, fact);
        id
    }

    pub fn add_definition(&mut self, symbol: SymbolId, draft: DefinitionDraft) -> DefinitionId {
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
        let stable_key_text = stable_key_input.stable_key();
        let fact = draft.into_fact(id, symbol, self.interner.intern(stable_key_text.clone()));

        self.record_id_key(IdFamily::Definition, id.0, &stable_key_text);
        self.insert_definition(stable_key_text, fact);
        id
    }

    pub fn add_reference(&mut self, target: SymbolId, draft: ReferenceDraft) -> ReferenceId {
        self.add_reference_with_status(
            Some(target),
            Vec::new(),
            SymbolResolutionStatus::Resolved,
            draft,
        )
    }

    pub fn add_ambiguous_reference(
        &mut self,
        mut candidates: Vec<SymbolId>,
        draft: ReferenceDraft,
    ) -> ReferenceId {
        candidates.sort_unstable();
        candidates.dedup();
        self.add_reference_with_status(None, candidates, SymbolResolutionStatus::Ambiguous, draft)
    }

    pub fn add_unresolved_reference(&mut self, draft: ReferenceDraft) -> ReferenceId {
        self.add_reference_with_status(None, Vec::new(), SymbolResolutionStatus::Unresolved, draft)
    }

    pub fn add_setup_missing_reference(
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

    pub fn add_setup_missing_reference_draft(&mut self, mut draft: ReferenceDraft) -> ReferenceId {
        draft.precision = SymbolPrecision::SetupMissing;
        self.add_reference_with_status(
            None,
            Vec::new(),
            SymbolResolutionStatus::SetupMissing,
            draft,
        )
    }

    #[cfg(test)]
    pub fn add_unsupported_reference(
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

    pub fn add_unsupported_reference_draft(&mut self, mut draft: ReferenceDraft) -> ReferenceId {
        draft.precision = SymbolPrecision::Unsupported;
        self.add_reference_with_status(None, Vec::new(), SymbolResolutionStatus::Unsupported, draft)
    }

    pub fn finish(self) -> SymbolGraphOutput {
        let mut symbols = self.symbols.into_values().collect::<Vec<_>>();
        let mut definitions = self.definitions.into_values().collect::<Vec<_>>();
        let mut references = self.references.into_values().collect::<Vec<_>>();
        symbols.sort_by(|left, right| symbol_order(&self.interner, left, right));
        definitions.sort_by(|left, right| definition_order(&self.interner, left, right));
        references.sort_by(|left, right| reference_order(&self.interner, left, right));

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
        let stable_key_text = stable_key_input.stable_key();
        let fact = draft.into_fact(
            id,
            target,
            candidates,
            self.interner.intern(stable_key_text.clone()),
            status,
        );

        self.record_id_key(IdFamily::Reference, id.0, &stable_key_text);
        self.insert_reference(stable_key_text, fact);
        id
    }

    fn insert_symbol(&mut self, stable_key_text: String, fact: SymbolFact) {
        if let Some(existing) = self.symbols.get(&stable_key_text) {
            if !same_symbol_fact(existing, &fact) {
                self.record_duplicate_diagnostic("symbol", fact.id.0, &stable_key_text);
            }
            return;
        }
        self.symbols.insert(stable_key_text, fact);
    }

    fn insert_definition(&mut self, stable_key_text: String, fact: DefinitionFact) {
        if let Some(existing) = self.definitions.get(&stable_key_text) {
            if !same_definition_fact(existing, &fact) {
                self.record_duplicate_diagnostic("definition", fact.id.0, &stable_key_text);
            }
            return;
        }
        self.definitions.insert(stable_key_text, fact);
    }

    fn insert_reference(&mut self, stable_key_text: String, fact: ReferenceFact) {
        if let Some(existing) = self.references.get(&stable_key_text) {
            if !same_reference_fact(existing, &fact) {
                self.record_duplicate_diagnostic("reference", fact.id.0, &stable_key_text);
            }
            return;
        }
        self.references.insert(stable_key_text, fact);
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

    fn into_fact(self, id: SymbolId, stable_key: StableKeyId) -> SymbolFact {
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

    fn into_fact(
        self,
        id: DefinitionId,
        symbol: SymbolId,
        stable_key: StableKeyId,
    ) -> DefinitionFact {
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
        stable_key: StableKeyId,
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

fn symbol_order(
    interner: &StableKeyInterner,
    left: &SymbolFact,
    right: &SymbolFact,
) -> std::cmp::Ordering {
    let left_stable_key = interner.resolve(left.stable_key);
    let right_stable_key = interner.resolve(right.stable_key);
    fact_order_key(
        &left_stable_key,
        left.file,
        left.primary_span.as_ref(),
        symbol_kind_rank(left.kind),
        &left.name,
    )
    .cmp(&fact_order_key(
        &right_stable_key,
        right.file,
        right.primary_span.as_ref(),
        symbol_kind_rank(right.kind),
        &right.name,
    ))
}

fn definition_order(
    interner: &StableKeyInterner,
    left: &DefinitionFact,
    right: &DefinitionFact,
) -> std::cmp::Ordering {
    let left_stable_key = interner.resolve(left.stable_key);
    let right_stable_key = interner.resolve(right.stable_key);
    fact_order_key(
        &left_stable_key,
        left.file,
        left.primary_span.as_ref(),
        definition_kind_rank(left.kind),
        &left.name,
    )
    .cmp(&fact_order_key(
        &right_stable_key,
        right.file,
        right.primary_span.as_ref(),
        definition_kind_rank(right.kind),
        &right.name,
    ))
}

fn reference_order(
    interner: &StableKeyInterner,
    left: &ReferenceFact,
    right: &ReferenceFact,
) -> std::cmp::Ordering {
    let left_stable_key = interner.resolve(left.stable_key);
    let right_stable_key = interner.resolve(right.stable_key);
    fact_order_key(
        &left_stable_key,
        left.file,
        left.primary_span.as_ref(),
        reference_kind_rank(left.kind),
        &left.name,
    )
    .cmp(&fact_order_key(
        &right_stable_key,
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
