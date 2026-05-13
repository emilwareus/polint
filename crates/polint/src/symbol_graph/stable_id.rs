use crate::cache::stable_hash;
use crate::core::{
    DefinitionId, Language, ReferenceId, Span, SymbolId, SymbolKind, SymbolNamespace,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableSymbolKey {
    language: Language,
    module_key: Option<String>,
    package_key: Option<String>,
    file_key: Option<String>,
    owner_chain: Vec<String>,
    namespace: SymbolNamespace,
    kind: SymbolKind,
    name: String,
    primary_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableDefinitionKey {
    symbol: StableSymbolKey,
    file_key: String,
    span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableReferenceKey {
    target: Option<StableSymbolKey>,
    language: Language,
    file_key: String,
    name: String,
    span: Span,
}

impl StableSymbolKey {
    #[expect(
        clippy::too_many_arguments,
        reason = "stable symbol keys are constructed from the normalized identity tuple"
    )]
    pub(crate) fn new(
        language: Language,
        module_key: Option<String>,
        package_key: Option<String>,
        file_key: Option<String>,
        owner_chain: Vec<String>,
        namespace: SymbolNamespace,
        kind: SymbolKind,
        name: String,
        primary_span: Option<Span>,
    ) -> Self {
        Self {
            language,
            module_key,
            package_key,
            file_key,
            owner_chain,
            namespace,
            kind,
            name,
            primary_span,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_namespace(&mut self, namespace: SymbolNamespace) {
        self.namespace = namespace;
    }

    #[cfg(test)]
    pub(crate) fn set_kind(&mut self, kind: SymbolKind) {
        self.kind = kind;
    }

    pub(crate) fn stable_key(&self) -> String {
        encode_tagged_parts(
            "symbol",
            [
                ("language", language_key(self.language).to_string()),
                ("module", optional_part(self.module_key.as_deref())),
                ("package", optional_part(self.package_key.as_deref())),
                ("file", optional_part(self.file_key.as_deref())),
                ("owner", encode_repeated_parts(&self.owner_chain)),
                ("namespace", namespace_key(self.namespace).to_string()),
                ("kind", kind_key(self.kind).to_string()),
                ("name", self.name.clone()),
                ("span", optional_span_part(self.primary_span.as_ref())),
            ],
        )
    }
}

impl StableDefinitionKey {
    pub(crate) fn new(symbol: StableSymbolKey, file_key: impl Into<String>, span: Span) -> Self {
        Self {
            symbol,
            file_key: file_key.into(),
            span,
        }
    }

    pub(crate) fn stable_key(&self) -> String {
        encode_tagged_parts(
            "definition",
            [
                ("symbol", self.symbol.stable_key()),
                ("file", self.file_key.clone()),
                ("span", span_part(&self.span)),
            ],
        )
    }
}

impl StableReferenceKey {
    pub(crate) fn resolved(
        target: StableSymbolKey,
        file_key: impl Into<String>,
        span: Span,
    ) -> Self {
        let language = target.language;
        let name = target.name.clone();
        Self {
            target: Some(target),
            language,
            file_key: file_key.into(),
            name,
            span,
        }
    }

    pub(crate) fn unresolved(
        language: Language,
        file_key: impl Into<String>,
        name: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            target: None,
            language,
            file_key: file_key.into(),
            name: name.into(),
            span,
        }
    }

    pub(crate) fn stable_key(&self) -> String {
        encode_tagged_parts(
            "reference",
            [
                ("target", optional_part(self.target_stable_key().as_deref())),
                ("language", language_key(self.language).to_string()),
                ("file", self.file_key.clone()),
                ("name", self.name.clone()),
                ("span", span_part(&self.span)),
            ],
        )
    }

    fn target_stable_key(&self) -> Option<String> {
        self.target.as_ref().map(StableSymbolKey::stable_key)
    }
}

pub(crate) type StableKeyHash = fn(&str) -> String;

pub(crate) fn default_stable_key_hash(stable_key: &str) -> String {
    stable_hash(&[stable_key])
}

#[cfg(test)]
pub(crate) fn symbol_id_from_key(key: &StableSymbolKey) -> SymbolId {
    symbol_id_from_key_with_hash(key, default_stable_key_hash)
}

#[cfg(test)]
pub(crate) fn definition_id_from_key(key: &StableDefinitionKey) -> DefinitionId {
    definition_id_from_key_with_hash(key, default_stable_key_hash)
}

#[cfg(test)]
pub(crate) fn reference_id_from_key(key: &StableReferenceKey) -> ReferenceId {
    reference_id_from_key_with_hash(key, default_stable_key_hash)
}

pub(crate) fn symbol_id_from_key_with_hash(key: &StableSymbolKey, hash: StableKeyHash) -> SymbolId {
    SymbolId(id_from_stable_key_with_hash(&key.stable_key(), hash))
}

pub(crate) fn definition_id_from_key_with_hash(
    key: &StableDefinitionKey,
    hash: StableKeyHash,
) -> DefinitionId {
    DefinitionId(id_from_stable_key_with_hash(&key.stable_key(), hash))
}

pub(crate) fn reference_id_from_key_with_hash(
    key: &StableReferenceKey,
    hash: StableKeyHash,
) -> ReferenceId {
    ReferenceId(id_from_stable_key_with_hash(&key.stable_key(), hash))
}

fn id_from_stable_key_with_hash(stable_key: &str, hash: StableKeyHash) -> u64 {
    digest_to_u64(&hash(stable_key))
}

fn digest_to_u64(digest: &str) -> u64 {
    let prefix = digest.get(..16).unwrap_or(digest);
    match u64::from_str_radix(prefix, 16) {
        Ok(value) => value,
        Err(_) => {
            let fallback = stable_hash(&[digest]);
            let fallback_prefix = fallback.get(..16).unwrap_or(fallback.as_str());
            u64::from_str_radix(fallback_prefix, 16)
                .unwrap_or_else(|_| deterministic_bytes_to_u64(fallback.as_bytes()))
        }
    }
}

fn deterministic_bytes_to_u64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn encode_tagged_parts<const N: usize>(kind: &str, parts: [(&str, String); N]) -> String {
    let mut encoded = length_prefixed(kind);
    for (label, value) in parts {
        encoded.push('|');
        encoded.push_str(label);
        encoded.push('=');
        encoded.push_str(&length_prefixed(&normalize_key_part(&value)));
    }
    encoded
}

fn length_prefixed(part: &str) -> String {
    format!("{}:{part}", part.len())
}

fn encode_repeated_parts(parts: &[String]) -> String {
    let mut encoded = String::new();
    for part in parts {
        if !encoded.is_empty() {
            encoded.push('/');
        }
        encoded.push_str(&length_prefixed(&normalize_key_part(part)));
    }
    encoded
}

fn normalize_key_part(part: &str) -> String {
    part.replace('\\', "/").replace('\n', "\\n")
}

fn optional_part(part: Option<&str>) -> String {
    part.unwrap_or("<none>").to_string()
}

fn optional_span_part(span: Option<&Span>) -> String {
    span.map_or_else(|| "<none>".to_string(), span_part)
}

fn span_part(span: &Span) -> String {
    format!(
        "file:{}:{}-{}:{}:{}-{}:{}",
        span.file.0,
        span.start_byte,
        span.end_byte,
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col
    )
}

fn language_key(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn namespace_key(namespace: SymbolNamespace) -> &'static str {
    match namespace {
        SymbolNamespace::Value => "value",
        SymbolNamespace::Type => "type",
        SymbolNamespace::Namespace => "namespace",
        SymbolNamespace::Package => "package",
        SymbolNamespace::Module => "module",
        SymbolNamespace::Unknown => "unknown",
    }
}

fn kind_key(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Package => "package",
        SymbolKind::Module => "module",
        SymbolKind::File => "file",
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Interface => "interface",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Enum => "enum",
        SymbolKind::EnumMember => "enum_member",
        SymbolKind::Variable => "variable",
        SymbolKind::Constant => "constant",
        SymbolKind::Parameter => "parameter",
        SymbolKind::Field => "field",
        SymbolKind::Property => "property",
        SymbolKind::Namespace => "namespace",
        SymbolKind::Import => "import",
        SymbolKind::Export => "export",
        SymbolKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod symbol_graph_stable_ids {
    use crate::core::{
        DefinitionId, Language, ReferenceId, Span, SymbolId, SymbolKind, SymbolNamespace,
    };
    use crate::symbol_graph::stable_id::{
        StableDefinitionKey, StableReferenceKey, StableSymbolKey, definition_id_from_key,
        reference_id_from_key, symbol_id_from_key,
    };

    fn span(start_byte: u32, end_byte: u32) -> Span {
        Span {
            file: crate::core::FileId(7),
            start_byte,
            end_byte,
            start_line: 3,
            start_col: 5,
            end_line: 3,
            end_col: 9,
        }
    }

    fn button_key() -> StableSymbolKey {
        StableSymbolKey::new(
            Language::TypeScript,
            Some("module:ui".to_string()),
            Some("package:web".to_string()),
            Some("src/button.ts".to_string()),
            vec!["Button".to_string()],
            SymbolNamespace::Value,
            SymbolKind::Function,
            "render".to_string(),
            Some(span(10, 16)),
        )
    }

    #[test]
    fn identical_symbol_keys_produce_identical_ids() {
        let left = button_key();
        let right = button_key();

        assert_eq!(symbol_id_from_key(&left), symbol_id_from_key(&right));
    }

    #[test]
    fn symbol_id_changes_when_semantic_namespace_changes() {
        let mut changed = button_key();
        changed.set_namespace(SymbolNamespace::Type);

        assert_ne!(
            symbol_id_from_key(&button_key()),
            symbol_id_from_key(&changed)
        );
    }

    #[test]
    fn symbol_id_changes_when_kind_changes() {
        let mut changed = button_key();
        changed.set_kind(SymbolKind::Class);

        assert_ne!(
            symbol_id_from_key(&button_key()),
            symbol_id_from_key(&changed)
        );
    }

    #[test]
    fn definition_id_changes_when_definition_span_changes() {
        let symbol = button_key();
        let first = StableDefinitionKey::new(symbol.clone(), "src/button.ts", span(10, 16));
        let second = StableDefinitionKey::new(symbol, "src/button.ts", span(20, 26));

        assert_ne!(
            definition_id_from_key(&first),
            definition_id_from_key(&second)
        );
    }

    #[test]
    fn reference_id_changes_when_reference_span_changes() {
        let target = button_key();
        let first = StableReferenceKey::resolved(target.clone(), "src/app.ts", span(30, 36));
        let second = StableReferenceKey::resolved(target, "src/app.ts", span(40, 46));

        assert_ne!(
            reference_id_from_key(&first),
            reference_id_from_key(&second)
        );
    }

    #[test]
    fn stable_keys_are_debug_safe_and_length_prefixed() {
        let key = button_key();
        let encoded = key.stable_key();

        assert!(encoded.contains("13:src/button.ts"));
        assert!(!encoded.contains("function Button()"));
    }

    #[test]
    fn stable_ids_change_when_part_boundaries_change() {
        let left = StableSymbolKey::new(
            Language::TypeScript,
            Some("ab".to_string()),
            Some("c".to_string()),
            Some("src/button.ts".to_string()),
            Vec::new(),
            SymbolNamespace::Value,
            SymbolKind::Function,
            "render".to_string(),
            Some(span(10, 16)),
        );
        let right = StableSymbolKey::new(
            Language::TypeScript,
            Some("a".to_string()),
            Some("bc".to_string()),
            Some("src/button.ts".to_string()),
            Vec::new(),
            SymbolNamespace::Value,
            SymbolKind::Function,
            "render".to_string(),
            Some(span(10, 16)),
        );

        assert_ne!(symbol_id_from_key(&left), symbol_id_from_key(&right));
    }

    #[test]
    fn stable_ids_use_core_newtypes() {
        let symbol = symbol_id_from_key(&button_key());
        let definition = definition_id_from_key(&StableDefinitionKey::new(
            button_key(),
            "src/button.ts",
            span(10, 16),
        ));
        let reference = reference_id_from_key(&StableReferenceKey::unresolved(
            Language::TypeScript,
            "src/app.ts",
            "missing",
            span(30, 36),
        ));

        assert!(matches!(symbol, SymbolId(_)));
        assert!(matches!(definition, DefinitionId(_)));
        assert!(matches!(reference, ReferenceId(_)));
    }
}
