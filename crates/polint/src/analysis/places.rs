use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{CallSiteId, MirBodyId, PlaceId};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis::types::facts::TypeShape;
use crate::analysis_kernel::FactFamily;
use crate::core::{FileId, FunctionId, Language, StableKeyId, StableKeyInterner, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceFact {
    pub(crate) id: PlaceId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) root: PlaceRoot,
    pub(crate) projections: Vec<PlaceProjection>,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: PlaceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PlaceTypeFact {
    pub(crate) place: PlaceId,
    pub(crate) ty: TypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PlaceRoot {
    Local {
        function: FunctionId,
        name: String,
    },
    Parameter {
        function: FunctionId,
        index: u32,
        name: Option<String>,
    },
    Global {
        symbol: Option<SymbolId>,
        name: String,
    },
    Temporary {
        body: MirBodyId,
        ordinal: u32,
    },
    CallReturn {
        call: CallSiteId,
    },
    Unknown {
        evidence: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PlaceProjection {
    Field(String),
    Property(String),
    IndexKnown(String),
    IndexUnknown { evidence: String },
    Deref,
    AwaitResult,
    CallReturn(CallSiteId),
    Unknown { evidence: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PlaceStatus {
    Resolved,
    Partial,
    Unknown,
    Unsupported,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct PlaceTableBuilder {
    places: BTreeMap<String, PlaceDraft>,
}

#[derive(Debug, Clone)]
pub(crate) struct PlaceInsert {
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) root: PlaceRoot,
    pub(crate) projections: Vec<PlaceProjection>,
    pub(crate) status: PlaceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceStableContext {
    file_key: String,
    function_key: String,
    body_key: String,
}

impl PlaceStableContext {
    pub(crate) fn new(
        file_key: impl Into<String>,
        function_key: impl Into<String>,
        body_key: impl Into<String>,
    ) -> Self {
        Self {
            file_key: file_key.into(),
            function_key: function_key.into(),
            body_key: body_key.into(),
        }
    }

    pub(crate) fn file_key(&self) -> &str {
        &self.file_key
    }

    pub(crate) fn body_key(&self) -> &str {
        &self.body_key
    }

    #[cfg(test)]
    fn for_test() -> Self {
        Self::new("test-file", "test-function", "test-body")
    }
}

impl PlaceTableBuilder {
    pub(crate) fn insert_with_context(
        &mut self,
        interner: &crate::core::StableKeyInterner,
        context: &PlaceStableContext,
        place: PlaceInsert,
    ) -> String {
        self.insert_typed_with_context(interner, context, place, None)
    }

    pub(crate) fn insert_typed_with_context(
        &mut self,
        interner: &crate::core::StableKeyInterner,
        context: &PlaceStableContext,
        place: PlaceInsert,
        ty: Option<TypeShape>,
    ) -> String {
        let stable_key = stable_key_for(
            interner,
            place.language,
            context,
            &place.root,
            &place.projections,
        );
        let draft = self
            .places
            .entry(stable_key.clone())
            .or_insert_with(|| PlaceDraft {
                language: place.language,
                file: place.file,
                function: place.function,
                root: place.root,
                projections: place.projections,
                ty: ty.clone(),
                status: place.status,
            });
        if draft.ty.is_none() {
            draft.ty = ty;
        }
        stable_key
    }

    pub(crate) fn finish_with_types(
        self,
        interner: &StableKeyInterner,
    ) -> (Vec<PlaceFact>, Vec<PlaceTypeFact>) {
        let mut place_types = Vec::new();
        let places = self
            .places
            .into_iter()
            .enumerate()
            .map(|(index, (stable_key, draft))| {
                let id = PlaceId(index as u64);
                if let Some(ty) = draft.ty {
                    place_types.push(PlaceTypeFact { place: id, ty });
                }
                PlaceFact {
                    id,
                    language: draft.language,
                    file: draft.file,
                    function: draft.function,
                    root: draft.root,
                    projections: draft.projections,
                    stable_key: interner.intern(stable_key),
                    status: draft.status,
                }
            })
            .collect();
        (places, place_types)
    }
}

#[derive(Debug, Clone)]
struct PlaceDraft {
    language: Language,
    file: Option<FileId>,
    function: Option<FunctionId>,
    root: PlaceRoot,
    projections: Vec<PlaceProjection>,
    ty: Option<TypeShape>,
    status: PlaceStatus,
}

fn stable_key_for(
    interner: &crate::core::StableKeyInterner,
    language: Language,
    context: &PlaceStableContext,
    root: &PlaceRoot,
    projections: &[PlaceProjection],
) -> String {
    let mut parts = vec![
        ("language".to_string(), format!("{language:?}")),
        ("file".to_string(), context.file_key.clone()),
        ("function".to_string(), context.function_key.clone()),
        (
            "projection_count".to_string(),
            projections.len().to_string(),
        ),
    ];
    add_root_parts(&mut parts, context, root);
    for (index, projection) in projections.iter().enumerate() {
        parts.push((projection_label(index), projection_part(projection)));
    }
    let borrowed_parts = parts
        .iter()
        .map(|(label, value)| (label.as_str(), value.clone()))
        .collect::<Vec<_>>();
    let stable_key = semantic_stable_key(interner, FactFamily::Place, &borrowed_parts);
    stable_key.into_string()
}

fn add_root_parts(
    parts: &mut Vec<(String, String)>,
    context: &PlaceStableContext,
    root: &PlaceRoot,
) {
    match root {
        PlaceRoot::Local { name, .. } => {
            parts.push(("root_kind".to_string(), "local".to_string()));
            parts.push(("root_name".to_string(), name.clone()));
        }
        PlaceRoot::Parameter { index, name, .. } => {
            parts.push(("root_kind".to_string(), "parameter".to_string()));
            parts.push(("parameter_index".to_string(), index.to_string()));
            parts.push(("root_name".to_string(), name.clone().unwrap_or_default()));
        }
        PlaceRoot::Global { name, .. } => {
            parts.push(("root_kind".to_string(), "global".to_string()));
            parts.push(("root_name".to_string(), name.clone()));
        }
        PlaceRoot::Temporary { ordinal, .. } => {
            parts.push(("root_kind".to_string(), "temporary".to_string()));
            parts.push(("root_body".to_string(), context.body_key.clone()));
            parts.push(("temporary_ordinal".to_string(), ordinal.to_string()));
        }
        PlaceRoot::CallReturn { call } => {
            parts.push(("root_kind".to_string(), "call_return".to_string()));
            parts.push(("call_start_byte".to_string(), call.0.to_string()));
        }
        PlaceRoot::Unknown { evidence } => {
            parts.push(("root_kind".to_string(), "unknown".to_string()));
            parts.push(("evidence".to_string(), evidence.clone()));
        }
    }
}

fn projection_label(index: usize) -> String {
    format!("projection_{index:06}")
}

fn projection_part(projection: &PlaceProjection) -> String {
    match projection {
        PlaceProjection::Field(name) => format!("field:{name}"),
        PlaceProjection::Property(name) => format!("property:{name}"),
        PlaceProjection::IndexKnown(index) => format!("index_known:{index}"),
        PlaceProjection::IndexUnknown { evidence } => format!("index_unknown:{evidence}"),
        PlaceProjection::Deref => "deref".to_string(),
        PlaceProjection::AwaitResult => "await_result".to_string(),
        PlaceProjection::CallReturn(call) => format!("call_return_start_byte:{}", call.0),
        PlaceProjection::Unknown { evidence } => format!("unknown:{evidence}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{CallSiteId, MirBodyId, PlaceId};
    use crate::core::{FileId, FunctionId, Language, SymbolId};

    fn single_place_key(root: PlaceRoot, projections: Vec<PlaceProjection>) -> String {
        let mut builder = PlaceTableBuilder::default();
        let interner = crate::core::StableKeyInterner::default();
        builder.insert_with_context(
            &interner,
            &PlaceStableContext::for_test(),
            PlaceInsert {
                language: Language::TypeScript,
                file: Some(FileId(1)),
                function: Some(FunctionId(10)),
                root,
                projections,
                status: PlaceStatus::Resolved,
            },
        );
        let stable_key = builder.finish_with_types(&interner).0.remove(0).stable_key;
        interner.resolve(stable_key).to_string()
    }

    #[test]
    fn place_stable_keys_distinguish_supported_root_kinds() {
        let roots = [
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "value".to_string(),
            },
            PlaceRoot::Parameter {
                function: FunctionId(10),
                index: 0,
                name: Some("value".to_string()),
            },
            PlaceRoot::Global {
                symbol: Some(SymbolId(3)),
                name: "value".to_string(),
            },
            PlaceRoot::Temporary {
                body: MirBodyId(4),
                ordinal: 0,
            },
            PlaceRoot::CallReturn {
                call: CallSiteId(5),
            },
            PlaceRoot::Unknown {
                evidence: "dynamic root".to_string(),
            },
        ];

        let mut keys = roots
            .into_iter()
            .map(|root| single_place_key(root, Vec::new()))
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();

        assert_eq!(keys.len(), 6);
    }

    #[test]
    fn place_projection_stable_keys_preserve_projection_order() {
        let ordered = single_place_key(
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "value".to_string(),
            },
            vec![
                PlaceProjection::Field("field".to_string()),
                PlaceProjection::Property("prop".to_string()),
                PlaceProjection::IndexKnown("0".to_string()),
                PlaceProjection::IndexUnknown {
                    evidence: "dynamic index".to_string(),
                },
                PlaceProjection::Deref,
                PlaceProjection::AwaitResult,
                PlaceProjection::CallReturn(CallSiteId(8)),
                PlaceProjection::Unknown {
                    evidence: "dynamic projection".to_string(),
                },
            ],
        );
        let reversed = single_place_key(
            PlaceRoot::Local {
                function: FunctionId(10),
                name: "value".to_string(),
            },
            vec![
                PlaceProjection::Unknown {
                    evidence: "dynamic projection".to_string(),
                },
                PlaceProjection::CallReturn(CallSiteId(8)),
                PlaceProjection::AwaitResult,
                PlaceProjection::Deref,
                PlaceProjection::IndexUnknown {
                    evidence: "dynamic index".to_string(),
                },
                PlaceProjection::IndexKnown("0".to_string()),
                PlaceProjection::Property("prop".to_string()),
                PlaceProjection::Field("field".to_string()),
            ],
        );

        assert_ne!(ordered, reversed);
        assert!(ordered.contains("projection_000000"));
        assert!(ordered.contains("projection_000007"));
    }

    #[test]
    fn place_table_builder_assigns_dense_ids_by_sorted_stable_key() {
        let interner = crate::core::StableKeyInterner::default();
        let mut builder = PlaceTableBuilder::default();
        builder.insert_with_context(
            &interner,
            &PlaceStableContext::for_test(),
            PlaceInsert {
                language: Language::Go,
                file: Some(FileId(1)),
                function: Some(FunctionId(10)),
                root: PlaceRoot::Local {
                    function: FunctionId(10),
                    name: "zeta".to_string(),
                },
                projections: Vec::new(),
                status: PlaceStatus::Resolved,
            },
        );
        builder.insert_with_context(
            &interner,
            &PlaceStableContext::for_test(),
            PlaceInsert {
                language: Language::Go,
                file: Some(FileId(1)),
                function: Some(FunctionId(10)),
                root: PlaceRoot::Local {
                    function: FunctionId(10),
                    name: "alpha".to_string(),
                },
                projections: Vec::new(),
                status: PlaceStatus::Resolved,
            },
        );

        let places = builder.finish_with_types(&interner).0;

        assert_eq!(
            places.iter().map(|place| place.id).collect::<Vec<_>>(),
            vec![PlaceId(0), PlaceId(1)]
        );
        let first = interner.resolve(places[0].stable_key);
        let second = interner.resolve(places[1].stable_key);
        assert!(first < second);
        assert!(!first.contains("place_id"));
        assert!(!second.contains("PlaceId"));
    }

    #[test]
    fn place_stable_keys_use_stable_context_not_dense_run_ids() {
        let context = PlaceStableContext::new(
            "src/auth.go",
            "function:auth.Authorize:stable",
            "body:auth.Authorize:stable",
        );
        let mut first = PlaceTableBuilder::default();
        let first_key = first.insert_with_context(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &context,
            PlaceInsert {
                language: Language::Go,
                file: Some(FileId(1)),
                function: Some(FunctionId(10)),
                root: PlaceRoot::Temporary {
                    body: MirBodyId(4),
                    ordinal: 12,
                },
                projections: vec![PlaceProjection::CallReturn(CallSiteId(8))],
                status: PlaceStatus::Partial,
            },
        );
        let mut second = PlaceTableBuilder::default();
        let second_key = second.insert_with_context(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &context,
            PlaceInsert {
                language: Language::Go,
                file: Some(FileId(99)),
                function: Some(FunctionId(77)),
                root: PlaceRoot::Temporary {
                    body: MirBodyId(44),
                    ordinal: 12,
                },
                projections: vec![PlaceProjection::CallReturn(CallSiteId(8))],
                status: PlaceStatus::Partial,
            },
        );

        assert_eq!(first_key, second_key);
        assert!(!first_key.contains("FileId"));
        assert!(!first_key.contains("FunctionId"));
        assert!(!first_key.contains("MirBodyId"));
        assert!(!first_key.contains("root_function"));
        assert!(!first_key.contains("root_symbol"));
    }
}
