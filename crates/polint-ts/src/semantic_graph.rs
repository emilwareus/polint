//! One-file TypeScript semantic extraction used by graph adapters.

use oxc_allocator::Allocator;
use oxc_semantic::SemanticBuilder;
use polint_analysis_api::SourceFile;
use polint_core::StableKeyInterner;

use crate::inventory::extract::{extract_ts_inventory_from_program, mark_inventory_partial_ast};
use crate::inventory::store::TsInventoryOutput;
use crate::object_model::extract::{
    extract_ts_object_model_from_program, mark_object_model_partial_ast,
};
use crate::object_model::store::TsObjectModelOutput;
use crate::parse::parse_ts_file;
use crate::scope::extract::{extract_ts_scope_from_program, mark_scope_partial_ast};
use crate::scope::store::TsScopeOutput;
use crate::token_flow::{TsTokenSourceFlow, collect_ts_token_source_flows_from_nodes};

#[derive(Debug)]
pub struct TsFileAnalysis {
    pub file: polint_core::FileId,
    pub inventory: TsInventoryOutput,
    pub scope: TsScopeOutput,
    pub object_model: TsObjectModelOutput,
    pub token_source_flows: Vec<TsTokenSourceFlow>,
}

pub fn analyze_ts_file(interner: &StableKeyInterner, file: &SourceFile) -> TsFileAnalysis {
    let source = file.source.as_ref();
    let allocator = Allocator::default();
    let parsed = parse_ts_file(&allocator, file);
    if parsed.is_catastrophic() {
        return TsFileAnalysis {
            file: file.id,
            inventory: TsInventoryOutput::default(),
            scope: TsScopeOutput::default(),
            object_model: TsObjectModelOutput::default(),
            token_source_flows: Vec::new(),
        };
    }

    let semantic = SemanticBuilder::new().build(parsed.program()).semantic;
    let nodes = semantic.nodes();
    let mut inventory =
        extract_ts_inventory_from_program(interner, file, source, parsed.program(), nodes)
            .normalized(interner);
    let mut scope = extract_ts_scope_from_program(
        interner,
        file,
        source,
        parsed.program(),
        semantic.scoping(),
        nodes,
    )
    .normalized(interner);
    let mut object_model = extract_ts_object_model_from_program(
        interner,
        file,
        source,
        parsed.program(),
        semantic.scoping(),
        nodes,
        &inventory,
    );
    if !parsed.fully_parsed {
        mark_inventory_partial_ast(&mut inventory);
        mark_scope_partial_ast(&mut scope);
        mark_object_model_partial_ast(&mut object_model);
    }
    let token_source_flows =
        collect_ts_token_source_flows_from_nodes(interner, file, &inventory, nodes);

    TsFileAnalysis {
        file: file.id,
        inventory,
        scope,
        object_model,
        token_source_flows,
    }
}
