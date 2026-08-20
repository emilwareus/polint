//! Compile-time fallbacks for TypeScript analysis implementation modules.

pub mod token_flow {
    use crate::analysis_api::SourceFile;
    use crate::ts::inventory::store::TsInventoryOutput;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct TsTokenSourceFlow {
        pub source_function_stable_key_text: String,
        pub callsite_stable_key_text: String,
        pub stable_key_text: String,
    }

    pub fn collect_ts_token_source_flows(
        _interner: &crate::internal_core::StableKeyInterner,
        _file: &SourceFile,
        _inventory: Option<&TsInventoryOutput>,
    ) -> Vec<TsTokenSourceFlow> {
        Vec::new()
    }
}

pub mod semantic_graph {
    use super::token_flow::TsTokenSourceFlow;
    use crate::analysis_api::SourceFile;
    use crate::ts::inventory::store::TsInventoryOutput;
    use crate::ts::object_model::store::TsObjectModelOutput;
    use crate::ts::scope::store::TsScopeOutput;

    #[derive(Debug)]
    pub struct TsFileAnalysis {
        pub file: crate::internal_core::FileId,
        pub inventory: TsInventoryOutput,
        pub scope: TsScopeOutput,
        pub object_model: TsObjectModelOutput,
        pub token_source_flows: Vec<TsTokenSourceFlow>,
    }

    pub fn analyze_ts_file(
        _interner: &crate::internal_core::StableKeyInterner,
        file: &SourceFile,
    ) -> TsFileAnalysis {
        TsFileAnalysis {
            file: file.id,
            inventory: TsInventoryOutput::default(),
            scope: TsScopeOutput::default(),
            object_model: TsObjectModelOutput::default(),
            token_source_flows: Vec::new(),
        }
    }
}
